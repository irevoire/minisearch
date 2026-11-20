# Minisearch

A real simple search engine just for funsies.

## Basic setup

Run the project with `cargo run --release`.

```
# add documents
echo '{ "id": 1, "text": "Hello World" }' | http ':3000/document'
echo '{ "id": 2, "text": "Hello Bob" }' | http ':3000/document'

# search
http ':3000/search?q=Hello'
http ':3000/search?q=Bob'
```

## Fragmentation benchmark

Indexes 70% of the movies dataset, then, for every cycle, deletes 10% of the dataset at random and
adds back 10% of the documents that aren't indexed, to see how the databases behave once fragmented.

```
# by default runs heed, sled, fiole and redb for 100 cycles
cargo run --release --bin fragmentation
# or pick the engines and the number of cycles
cargo run --release --bin fragmentation -- heed redb --cycles 500

# plot the results in a window, or in a png with `-e "output='fragmentation.png'"`
gnuplot fragmentation.gp
```

The timings and database sizes of every cycle are written to `fragmentation.dat`, see `--help` for
all the options.
