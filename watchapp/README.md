# Pest Stop Watchapp

Native C Pebble watchapp scaffold generated with `pebble new-project` and adapted for this repo.

## Build

```bash
pebble build
```

## Run In Emulator

```bash
pebble install --emulator basalt --logs build/watchapp.pbw
```

## Current State

- `src/c/watchapp.c`: simple mock transit UI
- `src/pkjs/index.js`: PebbleKit JS stub
- `package.json`: targets all post-Time platforms we care about

## Target Platforms

- `basalt`
- `chalk`
- `diorite`
- `flint`
- `emery`
- `gabbro`

## Next Step

Reintroduce `AppMessage` gradually in C, then connect PKJS to the Rust backend.
