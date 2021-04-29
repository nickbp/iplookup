# [THIS PROJECT HAS MOVED TO SOURCEHUT!](https://sr.ht/~nickbp/iplookup/)

I'm consolidating my personal projects in one place. As such the old GitHub repo is being archived, and work now continues on sourcehut.

# iplookup

Prints your public IP address to stdout by querying a specified [STUN](https://tools.ietf.org/html/rfc5389) server.

Useful for finding your public IP when behind a NAT or firewall. Uses an open standard that's faster and arguably safer than querying some rando's "what is my ip" website.

## Quickstart

```
$ cargo install iplookup
$ iplookup stun.l.google.com:19302
123.456.789.123
```

## Build

```
$ git clone git@github.com:nickbp/iplookup
$ cd iplookup
$ cargo build --release
$ ./target/release/iplookup stun.l.google.com:19302
123.456.789.123
```

Note: Building `iplookup` requires Rust 1.39.0 or later.

## Features

- To simplify scripting, the only thing written to stdout is the resulting public IP. Anything else goes to stderr.
- Automatic retries with exponential backoff, waiting a maximum of 31s for a response.
- Prints additional information about the request and response if the `DEBUG` environment variable is non-empty.

## License

This project is licensed under GPL 3 or any later version.
