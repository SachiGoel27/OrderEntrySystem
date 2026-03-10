#pragma once
#include "order.hpp"

class PriceLevel {
public:
    Price price = 0; // the price this level represents
    Order* head = nullptr; // oldest order (first to be filled)
    Order* tail = nullptr; // newest order
    int total_volume = 0; // total shares sitting at this price

    PriceLevel() = default;
    explicit PriceLevel(Price p) : price(p) {}


    void add(Order* order);
    void remove(Order* order);
    bool isEmpty() const {return head == nullptr; }
};