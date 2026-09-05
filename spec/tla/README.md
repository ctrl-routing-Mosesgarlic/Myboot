# TLA+ specification

`BootTransaction.tla` models the boot transaction (report §3.11). Check it with:

```
tlc BootTransaction.tla -config Bounded.cfg
```

Establishes `TypeOK`, `NoDeadEnd` (safety) and `EventuallyResolved` (liveness) on the bounded instance.
