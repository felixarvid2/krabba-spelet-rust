//! Ljudutgången på datorn: cpal spelar upp det som engine::audio blandar.
//! Saknas ljudkort eller ljudserver fortsätter spelet tyst.
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// Startar ljudströmmen. Strömmen måste leva lika länge som spelet.
pub fn start() -> Option<cpal::Stream> {
    let host = cpal::default_host();
    let dev = host.default_output_device()?;
    let cfg = dev.default_output_config().ok()?;
    let channels = cfg.channels() as usize;
    let rate = cfg.sample_rate().0 as f32;
    let config: cpal::StreamConfig = cfg.clone().into();
    let mut buf: Vec<f32> = vec![];
    // Synthen räknar i 44,1 kHz; enkel omsampling om enheten har en annan takt
    let step = super::audio::RATE / rate;
    let mut pos = 0f32;
    let mut have = 0usize;
    let mut fill = move |out: &mut [f32]| {
        let frames = out.len() / channels.max(1);
        for f in 0..frames {
            // buf har `have` stereoramar; pos är ett bråktal in i buf
            while pos as usize + 1 >= have {
                let keep = if have > 0 { [buf[(have - 1) * 2], buf[(have - 1) * 2 + 1]] } else { [0.0, 0.0] };
                let n = 512;
                buf.resize((n + 1) * 2, 0.0);
                super::audio::render(&mut buf[2..]);
                buf[0] = keep[0];
                buf[1] = keep[1];
                if have > 0 {
                    pos -= (have - 1) as f32;
                }
                have = n + 1;
            }
            let i = pos as usize;
            let t = pos - i as f32;
            let l = buf[i * 2] * (1.0 - t) + buf[(i + 1) * 2] * t;
            let r = buf[i * 2 + 1] * (1.0 - t) + buf[(i + 1) * 2 + 1] * t;
            for c in 0..channels {
                out[f * channels + c] = if c % 2 == 0 { l } else { r };
            }
            pos += step;
        }
    };
    let err = |e| eprintln!("Ljudfel: {e}");
    let stream = match cfg.sample_format() {
        cpal::SampleFormat::F32 => dev.build_output_stream(&config, move |d: &mut [f32], _| fill(d), err, None).ok()?,
        cpal::SampleFormat::I16 => {
            let mut tmp: Vec<f32> = vec![];
            dev.build_output_stream(
                &config,
                move |d: &mut [i16], _| {
                    tmp.resize(d.len(), 0.0);
                    fill(&mut tmp);
                    for (o, x) in d.iter_mut().zip(tmp.iter()) {
                        *o = (x.clamp(-1.0, 1.0) * 32767.0) as i16;
                    }
                },
                err,
                None,
            )
            .ok()?
        }
        cpal::SampleFormat::U16 => {
            let mut tmp: Vec<f32> = vec![];
            dev.build_output_stream(
                &config,
                move |d: &mut [u16], _| {
                    tmp.resize(d.len(), 0.0);
                    fill(&mut tmp);
                    for (o, x) in d.iter_mut().zip(tmp.iter()) {
                        *o = ((x.clamp(-1.0, 1.0) * 0.5 + 0.5) * 65535.0) as u16;
                    }
                },
                err,
                None,
            )
            .ok()?
        }
        _ => return None,
    };
    stream.play().ok()?;
    Some(stream)
}
