use std::io::{ErrorKind, Read};
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};

#[derive(Debug, Clone, Copy)]
pub struct VideoOptions {
    pub fps: u32,
    pub speed: f32,
    pub hwaccel: HwAccel,
}

impl VideoOptions {
    pub fn from_env() -> Self {
        let fps = std::env::var("KRC_VIDEO_FPS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(30);
        let speed = std::env::var("KRC_VIDEO_SPEED")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| *v > 0.0)
            .unwrap_or(1.0);
        let hwaccel = HwAccel::from_env();
        Self {
            fps,
            speed,
            hwaccel,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum HwAccel {
    Auto,
    None,
    Nvdec,
    Vaapi,
}

impl HwAccel {
    fn from_env() -> Self {
        match std::env::var("KRC_HWACCEL")
            .ok()
            .map(|v| v.to_ascii_lowercase())
            .as_deref()
        {
            Some("none") => Self::None,
            Some("nvdec") | Some("cuda") => Self::Nvdec,
            Some("vaapi") => Self::Vaapi,
            _ => Self::Auto,
        }
    }
}

pub enum FrameSource {
    None,
    Ffmpeg(FfmpegSource),
}

impl FrameSource {
    pub fn from_video_path(
        video_path: String,
        width: u32,
        height: u32,
        options: VideoOptions,
    ) -> Self {
        if !Path::new(&video_path).exists() {
            eprintln!("[rendercore] video path does not exist: {video_path}");
            return Self::None;
        }

        match FfmpegSource::new(
            video_path,
            width,
            height,
            options.fps,
            options.speed,
            options.hwaccel,
        ) {
            Ok(source) => Self::Ffmpeg(source),
            Err(err) => {
                eprintln!("[rendercore] ffmpeg source disabled: {err}");
                Self::None
            }
        }
    }

    pub fn fill_next_frame(&mut self, dst: &mut [u8]) -> bool {
        match self {
            Self::None => false,
            Self::Ffmpeg(source) => {
                if let Err(err) = source.fill_next_frame(dst) {
                    eprintln!("[rendercore] ffmpeg frame read failed: {err}");
                    false
                } else {
                    true
                }
            }
        }
    }
}

pub struct FfmpegSource {
    video_path: String,
    width: u32,
    height: u32,
    fps: u32,
    speed: f32,
    hwaccel: HwAccel,
    child: Child,
    stdout: ChildStdout,
}

impl FfmpegSource {
    fn new(
        video_path: String,
        width: u32,
        height: u32,
        fps: u32,
        speed: f32,
        hwaccel: HwAccel,
    ) -> Result<Self, String> {
        let (child, stdout) = spawn_ffmpeg(&video_path, width, height, fps, speed, hwaccel)?;
        println!(
            "[rendercore] ffmpeg source enabled path={} target={}x{}@{} speed={} hwaccel={:?}",
            video_path, width, height, fps, speed, hwaccel
        );
        Ok(Self {
            video_path,
            width,
            height,
            fps,
            speed,
            hwaccel,
            child,
            stdout,
        })
    }

    fn restart(&mut self) -> Result<(), String> {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let (child, stdout) = spawn_ffmpeg(
            &self.video_path,
            self.width,
            self.height,
            self.fps,
            self.speed,
            self.hwaccel,
        )?;
        self.child = child;
        self.stdout = stdout;
        Ok(())
    }

    fn fill_next_frame(&mut self, dst: &mut [u8]) -> Result<(), String> {
        if let Err(err) = self.stdout.read_exact(dst) {
            if err.kind() == ErrorKind::UnexpectedEof || err.kind() == ErrorKind::BrokenPipe {
                self.restart()?;
                self.stdout
                    .read_exact(dst)
                    .map_err(|e| format!("failed to read frame after restart: {e}"))?;
                return Ok(());
            }
            return Err(format!("failed to read ffmpeg frame: {err}"));
        }
        Ok(())
    }
}

impl Drop for FfmpegSource {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn spawn_ffmpeg(
    video_path: &str,
    width: u32,
    height: u32,
    fps: u32,
    speed: f32,
    hwaccel: HwAccel,
) -> Result<(Child, ChildStdout), String> {
    let vf = format!(
        "setpts=PTS/{speed:.4},fps={fps},scale={width}:{height}:force_original_aspect_ratio=increase,crop={width}:{height}"
    );

    let mut args = vec!["-hide_banner", "-loglevel", "error"];
    match hwaccel {
        HwAccel::Auto => args.extend(["-hwaccel", "auto"]),
        HwAccel::Nvdec => args.extend(["-hwaccel", "cuda"]),
        HwAccel::Vaapi => args.extend(["-hwaccel", "vaapi"]),
        HwAccel::None => {}
    }
    args.extend([
        "-stream_loop",
        "-1",
        "-i",
        video_path,
        "-an",
        "-sn",
        "-dn",
        "-vf",
        &vf,
        "-pix_fmt",
        "rgba",
        "-f",
        "rawvideo",
        "-",
    ]);

    let mut child = Command::new("ffmpeg")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("failed to spawn ffmpeg: {err}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "ffmpeg stdout is not piped".to_string())?;
    Ok((child, stdout))
}
