#pragma once
#include <cstdint>
#include <limits>
// Fixed point: $10.50 --> 1050
using Price = int64_t;
using Quantity = int32_t;
using OrderID = uint64_t;

const Price TICK_SIZE = 1; // tick size is 1 cent.
const Price NO_BID = 0;
const Price NO_ASK = std::numeric_limits<Price>::max();