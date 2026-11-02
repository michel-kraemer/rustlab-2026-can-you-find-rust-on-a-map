# Can you find Rust on a map?

> Companion code for my talk “Can you find Rust on a map?” at [RustLab 2026](https://rustlab.it/)

## Slides

<a href="https://michel-kraemer.github.io/rustlab-2026-can-you-find-rust-on-a-map/rustlab-2026-can-you-find-rust-on-a-map.pdf
"><img width="600" src="https://michel-kraemer.github.io/rustlab-2026-can-you-find-rust-on-a-map/rustlab-2026-can-you-find-rust-on-a-map.jpg" /></a>

https://michel-kraemer.github.io/rustlab-2026-can-you-find-rust-on-a-map/rustlab-2026-can-you-find-rust-on-a-map.pdf

## Prepare data

1. Download the OpenStreetMap data for Europe in `.osm.pbf` format from https://download.geofabrik.de/.
2. Run the data preparation part of the program to extract relevant objects and to write them to a [GeoJSON](https://geojson.org/) file:

    ```shell
    cargo run --release -- europe-latest.osm.pbf europe-latest-extracted.geojson
    ```

    **Note:** This may run several minutes!

## Run treasure hunt

Run the following command to start the treasure hunt:

```shell
cargo run --release -- hunt europe-latest-extracted.geojson
```
