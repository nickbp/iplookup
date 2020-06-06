#![deny(warnings, rust_2018_idioms)]

/*
    iplookup - Query STUN service for current public IP address
    Copyright (C) 2020  Nicholas Parker

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.
*/

use anyhow::{anyhow, bail, Context, Result};
use bytecodec::{DecodeExt, EncodeExt};
use rand::Rng;
use std::env;
use std::net::{SocketAddr, ToSocketAddrs};
use std::time::Duration;
use stun_codec::rfc5389::{methods, Attribute};
use stun_codec::{Message, MessageClass, MessageDecoder, MessageEncoder, TransactionId};
use tokio::net::UdpSocket;
use tokio::time;

fn print_syntax() {
    eprintln!(
        "Syntax: {} <host:port>\nEnv: DEBUG: enabled with any non-empty value",
        env::args().nth(0).unwrap()
    );
}

#[tokio::main]
async fn main() -> Result<()> {
    if env::args().len() <= 1 {
        print_syntax();
        bail!("Missing required argument");
    }
    let endpoint = env::args().nth(1).unwrap();
    if endpoint.starts_with('-') {
        // Probably a commandline argument like '-h'/'--help', avoid parsing as a hostname
        print_syntax();
        bail!("Unrecognized argument");
    }
    // Probably an endpoint, try to resolve it in case it's a hostname
    let addr = endpoint
        .to_socket_addrs()
        .with_context(|| format!("Invalid or unresolvable endpoint: {}", endpoint))?
        .next()
        .with_context(|| format!("Missing addresses in endpoint resolution: {}", endpoint))?;
    let local_addr = "0.0.0.0:0".to_socket_addrs()?.next().unwrap();
    let mut conn = UdpSocket::bind(local_addr).await?;

    // If the "DEBUG" envvar is non-empty, enable debug
    let debug = match env::var_os("DEBUG") {
        Some(val) => !val.is_empty(),
        None => false,
    };

    match run_client(&mut conn, &addr, debug).await {
        Ok(addr) => {
            println!("{}", addr.ip());
            Ok(())
        }
        Err(ioerr) => Err(ioerr),
    }
}

/// Runs the client: Sends a request and prints the response
async fn run_client(conn: &mut UdpSocket, dest: &SocketAddr, debug: bool) -> Result<SocketAddr> {
    // Build and send request
    let mut transaction_id_buf = [0u8; 12];
    rand::thread_rng().try_fill(&mut transaction_id_buf)?;
    let transaction_id = TransactionId::new(transaction_id_buf);
    let message =
        Message::<Attribute>::new(MessageClass::Request, methods::BINDING, transaction_id);
    if debug {
        eprintln!("Sending: {:#?}", &message);
    }
    let message_bytes = MessageEncoder::new()
        .encode_into_bytes(message)
        .context("Codec error when encoding request")?;

    // Wait for response, use arbitrarily large buf that shouldn't realistically be exceeded by UDP
    let mut recvbuf = [0u8; 2048];

    let recvsize = recv_exponential_backoff(conn, &dest, &message_bytes, &mut recvbuf).await?;

    let mut decoder = MessageDecoder::<Attribute>::new();
    let decoded = decoder
        .decode_from_bytes(&recvbuf[..recvsize])
        .context("Codec error when decoding response")?
        // Would use another .context() call, but BrokenMessage is incompatible.
        .map_err(|e| anyhow!("Message error when decoding response: {:?}", e))?;
    if debug {
        eprintln!("Received ({}b): {:#?}", recvsize, decoded);
    }

    // Check that the returned transaction ID matches what we sent
    if transaction_id != decoded.transaction_id() {
        bail!(
            "Returned transaction id {:?} doesn't match sent {:?}",
            decoded.transaction_id(),
            transaction_id
        );
    }

    let result = decoded
        .attributes()
        .filter_map(|a| {
            if let Attribute::MappedAddress(ma) = a {
                Some(ma.address())
            } else if let Attribute::XorMappedAddress(ma) = a {
                Some(ma.address())
            } else if let Attribute::XorMappedAddress2(ma) = a {
                Some(ma.address())
            } else {
                None
            }
        })
        .nth(0);

    result.with_context(|| format!("No address attribute found in response: {:?}", decoded))
}

async fn recv_exponential_backoff(
    conn: &mut UdpSocket,
    dest: &SocketAddr,
    sendbuf: &Vec<u8>,
    mut recvbuf: &mut [u8],
) -> Result<usize> {
    // Receive timeout durations: 1s, 2s, 4s, 8s, 16s (total wait: 31s)
    const RETRIES: u32 = 5;
    for timeout_exponent in 0..RETRIES {
        // (Re)send request. Shouldn't time out but just in case...
        let _sendsize =
            time::timeout(Duration::from_millis(1000), conn.send_to(sendbuf, dest)).await?;

        let timeout_ms = 1000 * 2_u64.pow(timeout_exponent);
        match time::timeout(
            Duration::from_millis(timeout_ms),
            conn.recv_from(&mut recvbuf),
        )
        .await
        {
            // Got a response from somewhere
            Ok(Ok((recvsize, recvdest))) => {
                // Before returning, check that the response is from who we're waiting for
                if *dest == recvdest {
                    return Ok(recvsize);
                }
                // If it doesn't match, resend and resume waiting, unless this was the last retry
                eprintln!(
                    "Response origin {:?} doesn't match request target {:?}",
                    recvdest, dest
                );
            }
            // A different error occurred, give up
            Ok(Err(e)) => Err(e).context("Failed to receive STUN response")?,
            // Timeout occurred, try again (or exit loop)
            Err(_) => {
                if timeout_exponent + 1 == RETRIES {
                    eprintln!("Timed out after {}ms, giving up.", timeout_ms);
                } else {
                    eprintln!("Timed out after {}ms, trying {} again...", timeout_ms, dest);
                }
            }
        }
    }
    bail!("Timed out waiting for response from {:?}", dest)
}
