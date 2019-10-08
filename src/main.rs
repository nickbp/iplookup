#![deny(warnings, rust_2018_idioms)]

use bytecodec::{DecodeExt, EncodeExt};
use rand::Rng;
use std::env;
use std::io;
use std::net::{SocketAddr, ToSocketAddrs};
use std::result::Result;
use std::time::Duration;
use stun_codec::{BrokenMessage, Message, MessageClass, MessageDecoder, MessageEncoder, TransactionId};
use stun_codec::rfc5389::{methods, Attribute};
use tokio::net::UdpSocket;
use tokio::timer::Timeout;

fn print_syntax() {
    eprintln!(
        "Syntax: {} <host:port>\nEnv: DEBUG: enabled with any non-empty value",
        env::args().nth(0).unwrap()
    );
}

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    if env::args().len() <= 1 {
        print_syntax();
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Missing required argument"));
    }
    let endpoint = env::args().nth(1).unwrap();
    if endpoint.starts_with('-') {
        // Probably a commandline argument like '-h'/'--help', avoid parsing as a hostname
        print_syntax();
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Unrecognized argument"));
    }
    // Probably an endpoint, try to resolve it in case it's a hostname
    let addr = resolve(endpoint)?;
    let local_addr = "0.0.0.0:0".to_socket_addrs()?.next().unwrap();
    let mut conn = UdpSocket::bind(local_addr).await?;

    // If the "DEBUG" envvar is non-empty, enable debug
    let debug = match env::var_os("DEBUG") {
        Some(val) => !val.is_empty(),
        None => false
    };

    match run_client(&mut conn, &addr, debug).await {
        Ok(addr) => {
            println!("{}", addr.ip());
            Ok(())
        }
        Err(ioerr) => Err(ioerr)
    }
}

fn resolve(endpoint: String) -> Result<SocketAddr, io::Error> {
    match endpoint.to_socket_addrs() {
        Ok(mut addrs) => Ok(addrs.next().expect("Missing addresses from OK response")),
        Err(_err) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Invalid or unresolvable UDP endpoint: {}", endpoint)
        ))
    }
}

/// Runs the client: Sends a request and prints the response
async fn run_client(conn: &mut UdpSocket, dest: &SocketAddr, debug: bool) -> Result<SocketAddr, io::Error> {
    // Build and send request
    let mut transaction_id_buf = [0u8; 12];
    rand::thread_rng().try_fill(&mut transaction_id_buf)?;
    let transaction_id = TransactionId::new(transaction_id_buf);
    let message = Message::<Attribute>::new(MessageClass::Request, methods::BINDING, transaction_id);
    if debug {
        eprintln!("Sending: {:?}", &message);
    }
    let message_bytes = MessageEncoder::new().encode_into_bytes(message).map_err(std_err_codec)?;

    // Wait for response, use arbitrarily large buf that shouldn't realistically be exceeded by UDP
    let mut recvbuf = [0u8; 2048];

    let recvsize = recv_exponential_backoff(conn, &dest, &message_bytes, &mut recvbuf).await?;

    let mut decoder = MessageDecoder::<Attribute>::new();
    let decoded = decoder.decode_from_bytes(&recvbuf[..recvsize])
        .map_err(std_err_codec)?
        .map_err(std_err_msg)?;
    if debug {
        eprintln!("Received ({}b): {:?}", recvsize, decoded);
    }

    // Check that the returned transaction ID matches what we sent
    if transaction_id != decoded.transaction_id() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Returned transaction id {:?} doesn't match sent {:?}", decoded.transaction_id(), transaction_id)
        ));
    }

    let result = decoded.attributes().filter_map(|a| {
        if let Attribute::MappedAddress(ma) = a {
            Some(ma.address())
        } else if let Attribute::XorMappedAddress(ma) = a {
            Some(ma.address())
        } else if let Attribute::XorMappedAddress2(ma) = a {
            Some(ma.address())
        } else {
            None
        }
    }).nth(0);

    match result {
        Some(addr) => {
            Ok(addr)
        },
        None => {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("No address attribute found in response: {:?}", decoded)
            ))
        }
    }
}

async fn recv_exponential_backoff(conn: &mut UdpSocket, dest: &SocketAddr, sendbuf: &Vec<u8>, mut recvbuf: &mut [u8]) -> Result<usize, io::Error> {
    // Receive timeout durations: 1s, 2s, 4s, 8s, 16s (total wait: 31s)
    const RETRIES: u32 = 5;
    for timeout_exponent in 0..RETRIES {
        // (Re)send request. Shouldn't time out but just in case...
        let _sendsize = Timeout::new(conn.send_to(sendbuf, dest), Duration::from_millis(1000)).await?;

        let timeout_ms = 1000 * 2_u64.pow(timeout_exponent);
        match Timeout::new(conn.recv_from(&mut recvbuf), Duration::from_millis(timeout_ms)).await {
            // Got a response from somewhere
            Ok(Ok((recvsize, recvdest))) => {
                // Before returning, check that the response is from who we're waiting for
                if *dest == recvdest {
                    return Ok(recvsize);
                }
                // If it doesn't match, resend and resume waiting, unless this was the last retry
                eprintln!("Response origin {:?} doesn't match request target {:?}", recvdest, dest);
            },
            // A different error occurred, give up
            Ok(Err(e)) => return Err(e),
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
    Err(io::Error::new(io::ErrorKind::TimedOut, format!("Timed out waiting for response from {:?}", dest)))
}

// Clean up after the library author's decision to obnoxiously reinvent the wheel with their own error types.
fn std_err_codec(from: bytecodec::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("Codec error: {:?}", from))
}

// Another day, another reinvented wheel
fn std_err_msg(from: BrokenMessage) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("Message error: {:?}", from))
}
