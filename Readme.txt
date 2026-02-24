git status: show what branch you are own

git -b checkout : create a branch and put yourself on the branch

git checkout (branch name) : go to the branch you created

to add:
git add . : (adds everything that you changed)
git commit -m "" : (in the paranthesis write what you wrote)
git push : (ensure it is on your branch not main)

do a pr on github.com to merge into main


current dsa ideas:

- order book with b-trees/avls so add is O(log n)
    - each book's price will point to a doubly linked list each node in the list
    is the order object

- order map, is a hash map O(1) removal
    - since we have a doubly linked list we can go to the exact spot in memory
    to remove the order.

- each ticker will have its own order book and order map.