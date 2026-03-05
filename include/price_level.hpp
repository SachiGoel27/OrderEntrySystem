#pragma once
#include "order.hpp"

class PriceLevel {
public:
    Order* head = nullptr; // oldest order (first to be filled)
    Order* tail = nullptr; // newest order
    int total_volume = 0; // total shares sitting at this price

    void add(Order* order);
    void remove(Order* order);
};