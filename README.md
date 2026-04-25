A toy lanugage that is functional, untyped and lazy, implemented following SPJ's book[^1].

`fib.toy`:

```
zipWith f l r = caseList l nil (zipWithOnConsL f r);
zipWithOnConsL f r headL tailL = caseList r nil (zipWithOnConsR f headL tailL);
zipWithOnConsR f headL tailL headR tailR = cons (f headL headR) (zipWith f tailL tailR);

add l r = l + r;

fibs = cons 0 (cons 1 (zipWith add fibs (tail fibs)));

index l i = if (i == 0) (head l) (index (tail l) (i - 1));  

take l n = caseList l nil (takeOnCons n);
takeOnCons n head tail = if (n == 0) nil (cons head (take tail (n - 1))); 

main = index (traceList (take fibs 51)) 50

```

`nix run . -- -i fib.toy -vvv`:

```log
2026-04-25T21:21:15.829+08:00 - DEBUG reading source from: fib.toy
2026-04-25T21:21:15.829+08:00 - DEBUG parsing
2026-04-25T21:21:15.830+08:00 - DEBUG done parsing
2026-04-25T21:21:15.830+08:00 - DEBUG constructing template instantiation machine
2026-04-25T21:21:15.831+08:00 - DEBUG done constructing template instantiation machine
2026-04-25T21:21:15.831+08:00 - DEBUG executing
2026-04-25T21:21:15.849+08:00 - DEBUG done executing
output: [0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144, 233, 377, 610, 987, 1597, 2584, 4181, 6765, 10946, 17711, 28657, 46368, 75025, 121393, 196418, 317811, 514229, 832040, 1346269, 2178309, 3524578, 5702887, 9227465, 14930352, 24157817, 39088169, 63245986, 102334155, 165580141, 267914296, 433494437, 701408733, 1134903170, 1836311903, 2971215073, 4807526976, 7778742049, 12586269025]
entry_node: Num(IntegerNode(12586269025))
```

[rust template](https://github.com/srid/rust-nix-template)

[^1]: [The implementation of functional programming languages, Simon Peyton Jones, Prentice Hall 1987](https://simon.peytonjones.org/slpj-book-1987/#errata)
