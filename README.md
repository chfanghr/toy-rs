A toy lanugage that is functional, untyped and lazy, implemented following SPJ's book[^1].

`fib.toy`:

```
zipWith f l r = caseList l nil (zipWithOnConsL f r);
zipWithOnConsL f r headL tailL = caseList r nil (zipWithOnConsR f headL tailL);
zipWithOnConsR f headL tailL headR tailR = cons (f headL headR) (zipWith f tailL tailR);

add l r = l + r;

fibs = cons 0 (cons 1 (zipWith add fibs (tail fibs)));

index l i = if (i == 0) then head l else index (tail l) (i - 1);  

take l n = caseList l nil (takeOnCons n);
takeOnCons n head tail = if (n == 0) then nil else cons head (take tail (n - 1)); 

main = index (traceList (take fibs 80)) 50

```

`nix run . -- -i fib.toy -v`:

```log
2026-05-31T19:49:34.705+08:00 - DEBUG reading source from: toy_programs/fib.toy
2026-05-31T19:49:34.705+08:00 - DEBUG parsing
2026-05-31T19:49:34.706+08:00 - DEBUG done parsing
2026-05-31T19:49:34.706+08:00 - DEBUG constructing template instantiation machine
2026-05-31T19:49:34.706+08:00 - DEBUG done constructing template instantiation machine
2026-05-31T19:49:34.706+08:00 - DEBUG executing
2026-05-31T19:49:34.709+08:00 - DEBUG gc triggered, heap size: 4102
2026-05-31T19:49:34.709+08:00 - DEBUG done gc, heap size: 430
2026-05-31T19:49:34.709+08:00 - DEBUG done executing
[0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144, 233, 377, 610, 987, 1597, 2584, 4181, 6765, 10946, 17711, 28657, 46368, 75025, 121393, 196418, 317811, 514229, 832040, 1346269, 2178309, 3524578, 5702887, 9227465, 14930352, 24157817, 39088169, 63245986, 102334155, 165580141, 267914296, 433494437, 701408733, 1134903170, 1836311903, 2971215073, 4807526976, 7778742049, 12586269025, 20365011074, 32951280099, 53316291173, 86267571272, 139583862445, 225851433717, 365435296162, 591286729879, 956722026041, 1548008755920, 2504730781961, 4052739537881, 6557470319842, 10610209857723, 17167680177565, 27777890035288, 44945570212853, 72723460248141, 117669030460994, 190392490709135, 308061521170129, 498454011879264, 806515533049393, 1304969544928657, 2111485077978050, 3416454622906707, 5527939700884757, 8944394323791464, 14472334024676221]
2026-05-31T19:49:34.709+08:00 - INFO entry_node: Num(IntegerNode(12586269025))
2026-05-31T19:49:34.709+08:00 - INFO stats: Stats { steps: 8858, peak_heap_size: 4102 }
```

[rust template](https://github.com/srid/rust-nix-template)

[^1]: [The implementation of functional programming languages, Simon Peyton Jones, Prentice Hall 1987](https://simon.peytonjones.org/slpj-book-1987/#errata)
