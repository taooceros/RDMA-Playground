#ifndef RefRingBuffer_H
#define RefRingBuffer_H
/*
The difference between `refringbuffer` and `ringbuffer` is that `refringbuffer`'s memory 
is referenced from another place,
 while `ringbuffer` has its own memory space allocated. 
*/

// pub struct RingBuf {
//     pub buf: u64,
//     pub ringMask: u32,
//     pub headtail: &'static [AtomicU32],
//     pub allocator: RingeBufAllocator,
// }
//A wait-free ring buffer provides a mechanism for relaying objects from one single "producer" thread to one single "consumer" thread without any locks.
#include <cstdint>
#include <atomic>
#include <array>
#include <memory>
#include <iomanip> 
#include <iostream>

template <typename T, std::size_t Max_Count>
class RefRingBuffer {
public:

    uint32_t write(const T* data, uint32_t writeSize) {
        uint64_t head = get_head_inbytes(std::memory_order_seq_cst);
        uint64_t tail = get_tail_inbytes(std::memory_order_seq_cst);
        uint32_t writePos = get_writepos_frombytes(tail);

        uint32_t available_space  = capacity_inbytes - (tail - head);

        if(writeSize > available_space)
            writeSize = available_space;

        //Important Note
        //Control write unit aligned as bytes_scale
        //When the buffer is full, 
        //the last space must always be left empty, resulting in cases where data cannot be completely written.
        if(bytes_scale != 1)
            writeSize = writeSize - (writeSize % bytes_scale);         

        uint32_t toEnd = capacity_inbytes - writePos;
        if(toEnd < writeSize) {
            memcpy((char*)buf_addr + writePos, data, toEnd * sizeof(T));
            memcpy((char*)buf_addr , data + toEnd, (writeSize - toEnd) * sizeof(T));
        }else{
            memcpy((char*)buf_addr+writePos, data, writeSize * sizeof(T));
        }
        set_tail_inbytes(tail + writeSize, std::memory_order_seq_cst);
        return writeSize;
    }

    /**
     * read for ring buffer
     *
     * @param data The pointer to store data
     * @param cnt if -1, read all availabe; if cnt > 0, read cnt items
     * @return bytesread
     */

    uint32_t read(T* data, int cnt = -1) {
        uint64_t head = get_head_inbytes(std::memory_order_seq_cst);
        uint64_t tail = get_tail_inbytes(std::memory_order_seq_cst);
        uint32_t available = tail - head;

        uint32_t readPos = get_readpos_frombytes(head);

        
        if(cnt>0 && available > cnt)
            available = cnt;
        
        //Important Note
        //Control read unit aligned as bytes_scale
        //To ensure that during MMIO (Memory-Mapped Input/Output), 
        //data is written out at a fixed unit size.
        if(bytes_scale != 1)
            available = available - (available % bytes_scale);

        if(available == 0) // No avaiable data, quick return
            return available;

        uint32_t toEnd = capacity_inbytes - readPos;
        if(toEnd < available){
            //has second
            memcpy(data, (char*)buf_addr+readPos, toEnd * sizeof(T));
            memcpy(data + toEnd, (char*)buf_addr, (available - toEnd) * sizeof(T));
        }else{
            memcpy(data, (char*)buf_addr+readPos, available * sizeof(T));
        }
        set_head_inbytes(head + available, std::memory_order_seq_cst);
        return available;
    }

    uint32_t calculate_distance_inbytes(uint32_t behind, uint32_t front){
        return  front - behind;
    }

    uint32_t calculate_bytesdistance_fromunit(uint32_t behind, uint32_t front){
        return  (front - behind) * bytes_scale;
    }

    uint32_t read_available_inbytes(){
        uint64_t head = get_head_inbytes(std::memory_order_seq_cst);
        uint64_t tail = get_tail_inbytes(std::memory_order_seq_cst);
        uint32_t available = tail - head;
        return available;
    }

    uint32_t write_available_inbytes(){
        uint64_t head = get_head_inbytes(std::memory_order_seq_cst);
        uint64_t tail = get_tail_inbytes(std::memory_order_seq_cst);
        uint32_t available  = capacity_inbytes - (tail - head);
        return available;
    }

    uint32_t get_capacity_inbytes(){
        return capacity_inbytes;
    }

    bool is_power_of_2(uint32_t x) {
        return ((x != 0) && ((x & (x - 1)) == 0));
    }

    int set_capacity_inunit(uint32_t val){
        if(!is_power_of_2(val)){
            std::cout << "Must set the value as the power of 2" << std::endl;
            return -1; 
        }else{
            capacity_inunit = val;
            ringMask = capacity_inunit - 1;
            capacity_inbytes = capacity_inunit * bytes_scale;
            return 0;
        }
    }

    void set_bytes_scale(uint32_t val){
        bytes_scale = val;
        capacity_inbytes = capacity_inunit * bytes_scale;
    }

    void set_buf_addr(void *addr){
        buf_addr = (std::array<T, Max_Count> *)addr;
    }

    void set_head_addr(void *addr){
        head_addr = (std::atomic<uint32_t> *)addr;
    }

    void set_tail_addr(void *addr){
        tail_addr = (std::atomic<uint32_t> *)addr;
    }

    void set_consumeReadData_addr(void *addr){
        consumeReadData = (std::atomic<uint64_t> *)addr;
    }

    uint64_t add_consumeReadData(uint64_t val){
        return consumeReadData->fetch_add(val, std::memory_order_seq_cst);
    }

    T* getBufAddress() {
        return buf_addr; 
    }

    inline uint32_t bound_headtail(uint32_t val){
        return val % capacity_inbytes;
    }

    inline uint64_t get_tail_inbytes(std::memory_order mem_order = std::memory_order_seq_cst){
        return tail_addr->load(mem_order) * (uint64_t)bytes_scale;
    }

    inline uint64_t get_head_inbytes(std::memory_order mem_order = std::memory_order_seq_cst){
        return head_addr->load(mem_order) * (uint64_t)bytes_scale;
    }

    inline void set_tail_inbytes(uint64_t val, std::memory_order mem_order = std::memory_order_seq_cst){
        return tail_addr->store(val / bytes_scale, mem_order);
    }

    inline void set_head_inbytes(uint64_t val, std::memory_order mem_order = std::memory_order_seq_cst){
        return head_addr->store(val / bytes_scale, mem_order);
    }

    inline uint32_t get_tail_inunit(std::memory_order mem_order = std::memory_order_seq_cst){
        return tail_addr->load(mem_order);
    }

    inline uint32_t get_head_inunit(std::memory_order mem_order = std::memory_order_seq_cst){
        return head_addr->load(mem_order);
    }

    inline void set_tail_inunit(uint32_t val, std::memory_order mem_order = std::memory_order_seq_cst){
        return tail_addr->store(val, mem_order);
    }

    inline void set_head_inunit(uint32_t val, std::memory_order mem_order = std::memory_order_seq_cst){
        return head_addr->store(val, mem_order);
    }

    uint32_t get_buf_offset_in_region(uint8_t* region_start){
       return  ((uint8_t*)buf_addr) - region_start;
    }

    uint32_t get_writepos_frombytes(uint64_t tail_inbytes){
        return tail_inbytes % capacity_inbytes;
        // using %, rather than & ,due to capacity_inbytes could be not power of 2
    }

    uint32_t get_readpos_frombytes(uint64_t head_inbytes){
        return head_inbytes % capacity_inbytes;
        // using %, rather than & ,due to capacity_inbytes could be not power of 2
    }

    uint32_t get_writepos_fromunit(uint32_t tail_inunit){
        //Important note: must convert tail_inbytes to uint64_t to avoid overflow
        uint64_t tail_inbytes = (uint64_t)tail_inunit * (uint64_t)bytes_scale;
        return tail_inbytes % capacity_inbytes;
        // using %, rather than & ,due to capacity_inbytes could be not power of 2
    }

    uint32_t get_readpos_fromunit(uint32_t head_inunit){
        //Important note: must convert tail_inbytes to uint64_t to avoid overflow
        uint64_t head_inbytes = (uint64_t)head_inunit * (uint64_t)bytes_scale;
        return head_inbytes % capacity_inbytes;
        // using %, rather than & ,due to capacity_inbytes could be not power of 2
    }

    void set_serve_ctrl(bool val){
        serve_ctrl = val;
    }

    void printBuf(size_t length = 200) {

        T* data = (unsigned char*) buf_addr;
        for (size_t i = 0; i < length; ++i) {
            std::cout << std::hex << std::setw(2) << std::setfill('0') << (unsigned int)(unsigned char)data[i] << " ";
        }
        std::cout << std::dec << std::endl; 
    }

    std::array<T, Max_Count> *buf_addr;
    uint8_t padding[1024*1024-16];
    //For example, when converting a byte stream into a ring queue, 
    //it is necessary to set a uint size for byte scaling.
    uint32_t bytes_scale = 1;
    //It is necessary to set the count as the actual capacity_inunit 
    //of the memory region pointed to by the pointer.
    //to avoid the buffer overflow
    uint32_t capacity_inunit = Max_Count;  // set capacity_inunit is as the unit size
    uint32_t capacity_inbytes = capacity_inunit * bytes_scale;
    uint32_t ringMask = Max_Count-1;
    std::atomic<uint32_t> *head_addr;
    std::atomic<uint32_t> *tail_addr;

    //ref from socket_buf.rs
    // used by RDMA data socket, used to sync with rdma remote peer for the local read buff free space size
    // when socket application consume data and free read buf space, it will fetch_add the value
    // if the value >= 0.5 of read buf, we will send the information to the remote peer immediately otherwise,
    // when rdmadata socket send data to peer, it will read and clear the consumeReadData and send the information
    // to the peer in the rdmawrite packet to save rdmawrite call
    bool require_consumeReadData = false;
    std::atomic<uint64_t> *consumeReadData;

    //This flag is used to indicate that this ring buffer serves the control channel.
    bool serve_ctrl = false;

    bool dma_pace = false;

    //Flag for snapshot to record
    bool hold_snapshot = false;
    uint32_t buffer_pos_for_snapshot = 0;

    // Data structure used to help DMA register doca mem buffer
    struct doca_buf *remote_doca_buf;
	struct doca_buf *local_doca_buf;
};
#endif