# iplookup

Prints your public IP address to stdout by querying a specified [STUN](https://tools.ietf.org/html/rfc5389) server.

Useful for finding your public IP when behind a NAT or firewall. Uses an open standard that's faster and arguably safer than querying some rando's "what is my ip" website.

## Quickstart

```
$ git clone git@github.com:nickbp/iplookup
$ cd iplookup
$ cargo build --release
$ ./target/release/iplookup stun.l.google.com:19302
123.456.789.123
```

Note: Building `iplookup` requires Rust 1.39.0 or later. This version is currently in beta and is [scheduled](https://blog.rust-lang.org/2019/09/30/Async-await-hits-beta.html) for general release on November 7th 2019.

## Features

- To simplify scripting, the only thing written to stdout is the resulting public IP. Anything else goes to stderr.
- Automatic retries with exponential backoff, waiting a maximum of 31s for a response.
- Prints additional information about the request and response if the `DEBUG` environment variable is non-empty.

## License

This project is licensed under GPL 3 or any later version.
