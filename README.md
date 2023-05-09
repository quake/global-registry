# global-registry

This is a demo contract that implements a global registry of config cell, which includes the two contracts:

1. `global-registry`: the global registry contract that manages the config cell in a linked list manner, ensuring that there is only one config cell in the chain. This contract should be used as a type script.

2. `lock-wrapper`: the lock wrapper contract that wraps the real lock script, coworking with the global registry contract, it will load the config value from the global registry contract, and then call the real lock script. This contract should be used as a lock script.

## How to build and test

Build contracts:

``` sh
capsule build
```

Run tests:

``` sh
capsule test
```
