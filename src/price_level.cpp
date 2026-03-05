#include "price_level.hpp"

// new order arrives we add it to the back (FIFO)
void PriceLevel::add(Order* order) {
    if (!order) return;

    if (head == nullptr) {
        head = order;
        tail = order;
        order->prev = nullptr;
        order->next = nullptr;
    } else {
        order->prev = tail;
        tail->next = order;
        order->next = nullptr;
        tail = order;
    }
    total_volume += order->qty;
}

void PriceLevel::remove(Order* order){
    if (!order) return;

    if (order->next != nullptr) {
        order->next->prev = order->prev;
    } else {
        tail = order->prev;
    }

    if (order->prev != nullptr) {
        order->prev->next = order->next;
    } else {
        head = order->next;
    }

    total_volume -= order->qty;

    order->next = nullptr;
    order->prev = nullptr;

}
