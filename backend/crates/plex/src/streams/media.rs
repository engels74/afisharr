// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The shape of a media entry, its parts, and its streams.

use serde::Deserialize;

use crate::wire::{Flag, optional_flag};

/// One media version of an item — a file, with its container-level facts.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaEntry {
    /// Plex's id for this version.
    #[serde(default)]
    pub id: Option<i64>,
    /// The container format, e.g. `mkv`.
    #[serde(default)]
    pub container: Option<String>,
    /// The resolution bucket, e.g. `4k`, `1080`.
    #[serde(default)]
    pub video_resolution: Option<String>,
    /// The video codec.
    #[serde(default)]
    pub video_codec: Option<String>,
    /// The video profile.
    #[serde(default)]
    pub video_profile: Option<String>,
    /// The frame rate bucket Plex reports, e.g. `24p`.
    #[serde(default)]
    pub video_frame_rate: Option<String>,
    /// The audio codec.
    #[serde(default)]
    pub audio_codec: Option<String>,
    /// How many audio channels the primary track has.
    #[serde(default)]
    pub audio_channels: Option<i32>,
    /// The aspect ratio.
    #[serde(default)]
    pub aspect_ratio: Option<f64>,
    /// The overall bitrate, in kbps as Plex reports it.
    #[serde(default)]
    pub bitrate: Option<i64>,
    /// Pixel width.
    #[serde(default)]
    pub width: Option<i32>,
    /// Pixel height.
    #[serde(default)]
    pub height: Option<i32>,
    /// Runtime in milliseconds.
    #[serde(default)]
    pub duration: Option<i64>,
    /// The files behind this version.
    #[serde(default, rename = "Part")]
    pub parts: Vec<MediaPart>,
}

/// One file of a media version.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaPart {
    /// Plex's id for this part.
    #[serde(default)]
    pub id: Option<i64>,
    /// The path on the server, as the server sees it.
    #[serde(default)]
    pub file: Option<String>,
    /// The file size in bytes.
    #[serde(default)]
    pub size: Option<i64>,
    /// The container format of this part.
    #[serde(default)]
    pub container: Option<String>,
    /// Plex's own report that the file can be read.
    ///
    /// `None` is "Plex did not say", which is not "the file is fine". The
    /// distinction is what makes a broken-media overlay honest rather than a
    /// badge that appears whenever an answer was short (PRD §13.2.5). A real
    /// server sends it only when the request asked for a file check, so `None`
    /// is the ordinary case rather than the exceptional one.
    #[serde(default, deserialize_with = "optional_flag")]
    pub accessible: Option<bool>,
    /// Plex's own report that the file is still there.
    #[serde(default, deserialize_with = "optional_flag")]
    pub exists: Option<bool>,
    /// The streams inside it.
    #[serde(default, rename = "Stream")]
    pub streams: Vec<MediaStream>,
}

/// What kind of stream this is.
///
/// Plex numbers them, and `Other` keeps a number this build has not seen rather
/// than dropping the stream: an unknown stream type is still a stream, and a
/// count that quietly excluded it would be wrong with no way to notice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamKind {
    /// A video track.
    Video,
    /// An audio track.
    Audio,
    /// A subtitle track.
    Subtitle,
    /// A lyric track.
    Lyric,
    /// Something else, keeping the number Plex used.
    Other(i64),
}

impl StreamKind {
    /// Reads Plex's `streamType`.
    #[must_use]
    pub const fn from_plex(value: i64) -> Self {
        match value {
            1 => Self::Video,
            2 => Self::Audio,
            3 => Self::Subtitle,
            4 => Self::Lyric,
            other => Self::Other(other),
        }
    }
}

/// The Dolby Vision attribute family.
///
/// A family and not a boolean, because profile 5 and profile 8.1 differ in
/// exactly the way an overlay pack wants to badge, and a single `dolbyVision`
/// flag cannot express it (PRD §13.2.5).
//
// The lint's usual remedy — collapse the flags into a state enum — is what this
// type exists to refuse: the four layer flags are four independent facts Plex
// reports separately, and an enum over them would be a vocabulary Afisharr
// invented on top of somebody else's.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DolbyVision {
    /// Whether any Dolby Vision layer is present.
    pub present: bool,
    /// The DV profile, when reported.
    pub profile: Option<i32>,
    /// The DV level, when reported.
    pub level: Option<i32>,
    /// Whether the base layer is present.
    pub base_layer: bool,
    /// Whether the enhancement layer is present.
    pub enhancement_layer: bool,
    /// Whether the RPU is present.
    pub rpu: bool,
    /// The base-layer compatibility id, when reported.
    pub compatibility_id: Option<i32>,
}

/// One stream inside a part.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaStream {
    /// What kind of stream it is.
    pub kind: StreamKind,
    /// The codec.
    pub codec: Option<String>,
    /// The language as Plex names it.
    pub language: Option<String>,
    /// The ISO language code.
    pub language_code: Option<String>,
    /// The stream's own title.
    pub title: Option<String>,
    /// Channel count, for audio.
    pub channels: Option<i32>,
    /// The channel layout, for audio.
    pub audio_channel_layout: Option<String>,
    /// Bit depth, for video and audio.
    pub bit_depth: Option<i32>,
    /// The colour space, for video.
    pub color_space: Option<String>,
    /// Whether a subtitle track is forced.
    pub forced: bool,
    /// Whether the track is marked for the hearing impaired.
    pub hearing_impaired: bool,
    /// Whether the track is marked for the visually impaired.
    pub visual_impaired: bool,
    /// The Dolby Vision family.
    pub dolby_vision: DolbyVision,
}

/// A stream exactly as Plex's JSON carries it.
//
// One field per attribute Plex sends, so the same reason applies as above and
// more plainly: this is a transcription of somebody else's wire format, and a
// state machine over it would be a second vocabulary to keep in step.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StreamBody {
    #[serde(default)]
    stream_type: i64,
    #[serde(default)]
    codec: Option<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    language_code: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    channels: Option<i32>,
    #[serde(default)]
    audio_channel_layout: Option<String>,
    #[serde(default)]
    bit_depth: Option<i32>,
    #[serde(default)]
    color_space: Option<String>,
    #[serde(default)]
    forced: Flag,
    #[serde(default)]
    hearing_impaired: Flag,
    #[serde(default)]
    visual_impaired: Flag,
    #[serde(default, rename = "DOVIPresent")]
    dovi_present: Flag,
    #[serde(default, rename = "DOVIProfile")]
    dovi_profile: Option<i32>,
    #[serde(default, rename = "DOVILevel")]
    dovi_level: Option<i32>,
    #[serde(default, rename = "DOVIBLPresent")]
    dovi_bl_present: Flag,
    #[serde(default, rename = "DOVIELPresent")]
    dovi_el_present: Flag,
    #[serde(default, rename = "DOVIRPUPresent")]
    dovi_rpu_present: Flag,
    #[serde(default, rename = "DOVIBLCompatID")]
    dovi_bl_compat_id: Option<i32>,
}

impl<'de> Deserialize<'de> for MediaStream {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let body = StreamBody::deserialize(deserializer)?;
        Ok(Self {
            kind: StreamKind::from_plex(body.stream_type),
            codec: body.codec,
            language: body.language,
            language_code: body.language_code,
            title: body.title,
            channels: body.channels,
            audio_channel_layout: body.audio_channel_layout,
            bit_depth: body.bit_depth,
            color_space: body.color_space,
            forced: body.forced.is_set(),
            hearing_impaired: body.hearing_impaired.is_set(),
            visual_impaired: body.visual_impaired.is_set(),
            dolby_vision: DolbyVision {
                present: body.dovi_present.is_set(),
                profile: body.dovi_profile,
                level: body.dovi_level,
                base_layer: body.dovi_bl_present.is_set(),
                enhancement_layer: body.dovi_el_present.is_set(),
                rpu: body.dovi_rpu_present.is_set(),
                compatibility_id: body.dovi_bl_compat_id,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_media_entry_reads_container_level_facts_and_its_parts() {
        let media: MediaEntry = serde_json::from_str(
            r#"{"id":7,"container":"mkv","videoResolution":"4k","videoCodec":"hevc",
                "audioCodec":"truehd","audioChannels":8,"bitrate":42000,"width":3840,
                "height":2160,"duration":8100000,
                "Part":[{"id":9,"file":"/data/Alien.mkv","size":123,"accessible":true,
                         "exists":true,"Stream":[]}]}"#,
        )
        .expect("parses");
        assert_eq!(media.video_resolution.as_deref(), Some("4k"));
        assert_eq!(media.audio_channels, Some(8));
        assert_eq!(media.parts[0].file.as_deref(), Some("/data/Alien.mkv"));
        assert_eq!(media.parts[0].accessible, Some(true));
    }

    #[test]
    fn a_part_that_says_nothing_about_the_file_is_unobserved_and_not_healthy() {
        // `accessible: None` is Plex not answering. Defaulted to `true` it is a
        // doctor page reporting every unanalysed file as fine (P1).
        let media: MediaEntry = serde_json::from_str(r#"{"Part":[{"id":1}]}"#).expect("parses");
        assert_eq!(media.parts[0].accessible, None);
        assert_eq!(media.parts[0].exists, None);
    }

    #[test]
    fn the_dolby_vision_family_survives_as_more_than_a_boolean() {
        let media: MediaEntry = serde_json::from_str(
            r#"{"Part":[{"Stream":[{"streamType":1,"codec":"hevc","DOVIPresent":true,
                "DOVIProfile":8,"DOVIBLCompatID":1,"DOVIRPUPresent":true}]}]}"#,
        )
        .expect("parses");
        let stream = &media.parts[0].streams[0];
        assert_eq!(stream.kind, StreamKind::Video);
        assert!(stream.dolby_vision.present);
        assert_eq!(stream.dolby_vision.profile, Some(8));
        assert_eq!(stream.dolby_vision.compatibility_id, Some(1));
        assert!(stream.dolby_vision.rpu);
    }

    #[test]
    fn an_audio_stream_reads_its_language_and_layout() {
        let media: MediaEntry = serde_json::from_str(
            r#"{"Part":[{"Stream":[{"streamType":2,"codec":"dts","language":"English",
                "languageCode":"eng","channels":6,"audioChannelLayout":"5.1"}]}]}"#,
        )
        .expect("parses");
        let stream = &media.parts[0].streams[0];
        assert_eq!(stream.kind, StreamKind::Audio);
        assert_eq!(stream.language_code.as_deref(), Some("eng"));
        assert_eq!(stream.audio_channel_layout.as_deref(), Some("5.1"));
    }

    #[test]
    fn a_forced_subtitle_track_says_so() {
        let media: MediaEntry = serde_json::from_str(
            r#"{"Part":[{"Stream":[{"streamType":3,"forced":true,"hearingImpaired":true}]}]}"#,
        )
        .expect("parses");
        let stream = &media.parts[0].streams[0];
        assert_eq!(stream.kind, StreamKind::Subtitle);
        assert!(stream.forced);
        assert!(stream.hearing_impaired);
    }

    #[test]
    fn a_stream_type_this_build_has_not_seen_keeps_its_number() {
        assert_eq!(StreamKind::from_plex(9), StreamKind::Other(9));
    }

    #[test]
    fn every_flag_reads_the_same_in_every_spelling_a_server_uses() {
        // All of these are XML attributes underneath, and a strict `bool` did
        // not read the wrong value — it failed the whole item parse.
        let media: MediaEntry = serde_json::from_str(
            r#"{"Part":[{"accessible":"1","exists":0,
                "Stream":[{"streamType":3,"forced":"1","hearingImpaired":1,
                           "DOVIPresent":"true"}]}]}"#,
        )
        .expect("parses");
        assert_eq!(media.parts[0].accessible, Some(true));
        assert_eq!(media.parts[0].exists, Some(false));
        assert!(media.parts[0].streams[0].forced);
        assert!(media.parts[0].streams[0].hearing_impaired);
        assert!(media.parts[0].streams[0].dolby_vision.present);
    }
}
