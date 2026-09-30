cat << 'PATCH' > src/ff_commands.rs.patch
<<<<<<< SEARCH
    #[test]
    fn test_ffprobe_get_format_tags_file_not_found() {
=======
    #[test]
    fn test_ffprobe_get_format_tags_file_not_found() {
        let non_existent_path = Path::new("does_not_exist_xyz.mp4");

        let output = ffprobe_get_format_tags(non_existent_path, "creation_time")
            .expect("failed to execute ffprobe_get_format_tags");

        assert!(!output.status.success());
    }

    #[test]
    fn test_ffprobe_find_telemetry_stream_success() {
        // Create a dummy video file first.
        let temp_video = tempfile::Builder::new()
            .suffix(".mp4")
            .tempfile()
            .expect("Failed to create temp video file");

        let status = Command::new("ffmpeg")
            .arg("-y")
            .arg("-f")
            .arg("lavfi")
            .arg("-i")
            .arg("color=c=black:s=10x10:d=1")
            .arg("-c:v")
            .arg("libx264")
            .arg(temp_video.path())
            .status()
            .expect("Failed to run ffmpeg to create dummy video");
        assert!(status.success(), "ffmpeg command failed to create dummy video");

        // Now multiplex a dummy telemetry stream into it.
        let dummy_bin = tempfile::Builder::new()
            .suffix(".bin")
            .tempfile()
            .expect("Failed to create temp bin file");
        std::fs::write(dummy_bin.path(), b"dummy telemetry").unwrap();

        let temp_muxed = tempfile::Builder::new()
            .suffix(".mp4")
            .tempfile()
            .expect("Failed to create temp muxed video file");

        // FFmpeg command to multiplex data stream with a custom codec tag.
        // We use -map 0:v -map 1:d -c copy, and very importantly we force the tag:
        // -tag:d gpmd which is GoPro's telemetry format tag, or similar to avoid codec id errors.
        // However, generic MP4 multiplexing with generic data stream can fail.
        // For testing ffprobe output, we just need ANY data stream. We can just use raw data format if needed,
        // or a simpler format like mkv that accepts any data stream. Let's use mkv.
        let temp_muxed_mkv = tempfile::Builder::new()
            .suffix(".mkv")
            .tempfile()
            .expect("Failed to create temp muxed mkv file");

        // Actually wait, mkv also complains "No bmp codec tag found for codec wrapped_avframe".
        // Let's create an mkv properly:
        let status = Command::new("ffmpeg")
            .arg("-y")
            .arg("-f")
            .arg("lavfi")
            .arg("-i")
            .arg("color=c=black:s=10x10:d=1")
            .arg("-c:v")
            .arg("libx264")
            .arg(temp_muxed_mkv.path())
            .status()
            .expect("Failed to run ffmpeg to create dummy video");
        assert!(status.success(), "ffmpeg command failed to create dummy mkv video");

        // Now attach a subtitle stream and pretend it's our target?
        // Wait, ffprobe_find_telemetry_stream specifically selects `-select_streams d` (data streams).
        // Let's create an mp4 with a compatible data stream. A `gpmd` or `tmcd` (timecode) stream.
        // Timecode is easier to generate: `-timecode 00:00:00:00`
        let temp_timecode = tempfile::Builder::new()
            .suffix(".mp4")
            .tempfile()
            .expect("Failed to create temp timecode video file");

        let status = Command::new("ffmpeg")
            .arg("-y")
            .arg("-f")
            .arg("lavfi")
            .arg("-i")
            .arg("color=c=black:s=10x10:d=1")
            .arg("-c:v")
            .arg("libx264")
            .arg("-timecode")
            .arg("00:00:00:00")
            .arg(temp_timecode.path())
            .status()
            .expect("Failed to create video with timecode data stream");
        assert!(status.success(), "ffmpeg failed to create timecode stream");

        let output = ffprobe_find_telemetry_stream(
            temp_timecode.path().to_str().expect("valid utf8 path")
        ).expect("failed to execute ffprobe_find_telemetry_stream");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        // The output should contain the stream index and codec tag. For timecode in mp4, it's typically 'tmcd'
        assert!(stdout.contains("tmcd"));
    }

    #[test]
    fn test_ffmpeg_dump_telemetry_success() {
        let temp_timecode = tempfile::Builder::new()
            .suffix(".mp4")
            .tempfile()
            .expect("Failed to create temp timecode video file");

        let status = Command::new("ffmpeg")
            .arg("-y")
            .arg("-f")
            .arg("lavfi")
            .arg("-i")
            .arg("color=c=black:s=10x10:d=1")
            .arg("-c:v")
            .arg("libx264")
            .arg("-timecode")
            .arg("00:00:00:00")
            .arg(temp_timecode.path())
            .status()
            .expect("Failed to create video with timecode data stream");
        assert!(status.success());

        let dump_out = tempfile::Builder::new()
            .suffix(".bin")
            .tempfile()
            .expect("Failed to create temp dump bin");

        // The timecode stream is usually at index 1 or 0 depending on muxing, we can use "d:0" mapping in test or just "1" if we know it.
        // To be safe we could query it first, but ffmpeg dump telemetry accepts "0:d:0" or just "d:0". Wait, the function takes `stream_idx` directly. So we can pass "d:0".
        let exit_status = ffmpeg_dump_telemetry(
            temp_timecode.path().to_str().expect("valid utf8 path"),
            "d:0",
            dump_out.path()
        ).expect("failed to dump telemetry");

        assert!(exit_status.success());
        // Verify output file has content
        let meta = std::fs::metadata(dump_out.path()).unwrap();
        assert!(meta.len() > 0);
    }
>>>>>>> REPLACE
PATCH
patch src/ff_commands.rs < src/ff_commands.rs.patch
