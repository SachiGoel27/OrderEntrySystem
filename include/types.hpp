#pragma once
#include <cstdint>
#include <limits>
// Fixed-point: monetary amount = dollars * PRICE_SCALE (e.g. $10.50 -> 10_500_000).
// Supports micro-tick style precision on the wire (int64 ticks).
using Price = int64_t;
using Quantity = int32_t;
using OrderID = uint64_t;

inline constexpr Price PRICE_SCALE = 1'000'000LL;
/// Smallest representable price increment in internal units (= $1 / PRICE_SCALE).
inline constexpr Price TICK_SIZE = 1;
inline constexpr Price NO_BID = 0;
inline constexpr Price NO_ASK = std::numeric_limits<Price>::max();

inline double priceDisplay(Price p) {
    return static_cast<double>(p) / static_cast<double>(PRICE_SCALE);
}
