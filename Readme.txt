current dsa ideas:

- order book with b-trees/avls so add is O(log n)
    - each book's price will point to a doubly linked list each node in the list
    is the order object

- order map, is a hash map O(1) removal
    - since we have a doubly linked list we can go to the exact spot in memory
    to remove the order.

- each ticker will have its own order book and order map.