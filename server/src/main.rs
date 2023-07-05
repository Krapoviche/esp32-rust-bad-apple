use std::fs::File;
use std::{thread, env};
use std::path::Path;
use std::net::{ TcpListener, TcpStream, Shutdown};
use std::io::{ Read, Write };
use ffmpeg::frame::Video;
use ffmpeg_next as ffmpeg;
use ffmpeg::format::{input, Pixel};
use ffmpeg::media::{Type};
use ffmpeg::software::scaling::{context::Context, flag::Flags};

fn main()  {
    ffmpeg::init().unwrap();

    let listener = TcpListener::bind("192.168.1.180:6767").unwrap();
    // accept connections and process them, spawning a new thread for each one
    println!("Server listening on port 6767");
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("New connection: {}", stream.peer_addr().unwrap());
                thread::spawn(move|| {
                    // connection succeeded
                    handle_client(stream)
                });
            }
            Err(e) => {
                println!("Error: {}", e);
                /* connection failed */
            }
        }
    }
    // close the socket server
    drop(listener);
}


fn handle_client(mut tcpstream: TcpStream) -> Result<(), ffmpeg::Error> {


    println!("{:?}",&env::args().nth(1));

    if let Ok(mut ictx) = input(&env::args().nth(1).expect("Cannot open file.")) {
        let input = ictx
            .streams()
            .best(Type::Video)
            .ok_or(ffmpeg::Error::StreamNotFound)?;
        let video_stream_index = input.index();

        let context_decoder = ffmpeg::codec::context::Context::from_parameters(input.parameters())?;
        let mut decoder = context_decoder.decoder().video()?;

        let mut scaler = Context::get(
            decoder.format(),
            decoder.width(),
            decoder.height(),
            Pixel::RGB24,
            decoder.width(),
            decoder.height(),
            Flags::BILINEAR,
        )?;

        let mut frame_index = 0;

        let mut receive_and_process_decoded_frames =
            |decoder: &mut ffmpeg::decoder::Video| -> Result<(), ffmpeg::Error> {
                let mut decoded = Video::empty();
                while decoder.receive_frame(&mut decoded).is_ok() {
                    let mut rgb_frame = Video::empty();
                    scaler.run(&decoded, &mut rgb_frame)?;
                    println!("sending frame n° {} with len {} to {}", frame_index, &rgb_frame.data(0).len(),tcpstream.peer_addr().unwrap());
                    let mut new_frame = transform_frame(&rgb_frame);
                    tcpstream.write(&new_frame).unwrap();
                    save_file(&rgb_frame, frame_index).unwrap();
                    frame_index += 1;
                }
                Ok(())
            };

        for (stream, packet) in ictx.packets() {
            if stream.index() == video_stream_index {
                decoder.send_packet(&packet)?;
                println!("decode");
                receive_and_process_decoded_frames(&mut decoder)?;
            }
        }
        decoder.send_eof()?;
        receive_and_process_decoded_frames(&mut decoder)?;
    }
    tcpstream.shutdown(Shutdown::Both).unwrap();
    Ok(())
}


fn save_file(frame: &Video, index: usize) -> std::result::Result<(), std::io::Error> {
    let mut file = File::create(format!("frames/frame{}.ppm", index))?;
    file.write_all(format!("P6\n{} {}\n255\n", frame.width(), frame.height()).as_bytes())?;
    file.write_all(frame.data(0))?;
    Ok(())
}

fn transform_frame(frame: &Video) -> [u8; 1024] {
    let mut new_frame: [u8; 1024] = [0 as u8; 1024];

    for i in 0..&frame.data(0).len()-1{
        if i%3 == 0 {
            let greyscale: u8 = &frame.data(0)[i] / 3 + &frame.data(0)[i+1] / 3 + &frame.data(0)[i+2]/ 3 ;
            let mut will_it_be_white: u8 = 0;
            if greyscale > 0xD0 {
                will_it_be_white = 1;
                new_frame[i/24] += will_it_be_white * 2u8.pow( ((i/3) %8) as u32);
            }
        }
    }

    return new_frame;
}
