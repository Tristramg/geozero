# GeoZero

Zero-Copy reading and writing of geospatial data.

GeoZero defines an API for reading geospatial data formats without an intermediate representation.
It defines traits which can be implemented to read and convert to an arbitrary format
or render geometries directly.

Supported geometry types:
* [OGC Simple Features](https://en.wikipedia.org/wiki/Simple_Features)
* (Planned) Circular arcs as defined by SQL-MM Part 3.
* (Planned) TIN

Supported dimensions: X, Y, Z, M, T

## CLI

geozero includes a command line interface for converting date between supported formats.

## Available implementations

* GeoJSON Reader + Writer
* [FlatGeobuf Reader](https://github.com/bjornharrtell/flatgeobuf)
* WKT Writer
* SVG Writer
* [geo](https://github.com/georust/geo) Writer
