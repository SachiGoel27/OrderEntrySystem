#pragma once
#include "types.hpp"

enum class Side { BUY, SELL };

struct Order {
    OrderID id;
    Side side;
    Price price;
    Quantity qty;

    // pointers for doubly linked list (can change this to array backed queue later if needed)
    Order* next = nullptr;
    Order* prev = nullptr;
};