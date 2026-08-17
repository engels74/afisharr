// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! One item's media, its parts, and the streams inside them.

use crate::fake::{element::Element, shape::Detail, state::FakeItem};

/// The `Media` element of one item.
pub(crate) fn media(item: &FakeItem, detail: Detail) -> Element {
    let mut entry = Element::named("Media")
        .number("id", 1_i64)
        .text("container", "mkv")
        .text("videoResolution", "1080")
        .text("videoCodec", "h264")
        .text("audioCodec", "eac3")
        .number("audioChannels", 6_i64)
        .number("bitrate", 8000_i64)
        .number("width", 1920_i64)
        .number("height", 1080_i64)
        .number("duration", 7_200_000_i64);
    if !detail.withhold {
        // Attributes a real server sends and this fake used to omit, so the
        // parsers that read them were checked against nothing at all.
        entry = entry
            .decimal("aspectRatio", 1.78)
            .text("videoProfile", "high")
            .text("videoFrameRate", "24p");
    }
    entry.child(part(item, detail))
}

/// The `Part` element — one file of one media version.
fn part(item: &FakeItem, detail: Detail) -> Element {
    let mut file = Element::named("Part")
        .number("id", 1_i64)
        .text("key", "/library/parts/1/1700000000/file.mkv")
        .text("file", format!("/data/{}.mkv", item.rating_key))
        .number("size", 4_000_000_000_i64)
        .text("container", "mkv");
    // Both require Plex to go and look at the file, which it does only when the
    // request asks (`plexapi/media.py:110-112`). Sent unconditionally, the
    // `None` case the client's own documentation is written against never
    // happens — and a broken-media overlay cannot be shown to be honest.
    if detail.check_files {
        file = file.flag("accessible", true).flag("exists", true);
    }
    file.children(streams(detail))
}

/// The `Stream` elements inside one part.
fn streams(detail: Detail) -> Vec<Element> {
    let video = Element::named("Stream")
        .number("streamType", 1_i64)
        .text("codec", "h264")
        .number("bitDepth", 8_i64)
        .text("colorSpace", "bt709");
    let audio = Element::named("Stream")
        .number("streamType", 2_i64)
        .text("codec", "eac3")
        .number("channels", 6_i64)
        .text("audioChannelLayout", "5.1")
        .text("language", "English")
        .text("languageCode", "eng");
    let subtitle = Element::named("Stream")
        .number("streamType", 3_i64)
        .text("codec", "subrip")
        .text("language", "English")
        .text("languageCode", "eng")
        .flag("forced", false);

    let titles = ["English (H.264)", "English (EAC3 5.1)", "English (SRT)"];
    [video, audio, subtitle]
        .into_iter()
        .enumerate()
        .map(|(position, stream)| {
            if detail.withhold {
                return stream;
            }
            let index = i64::try_from(position).unwrap_or(0);
            stream
                .number("id", index + 1)
                .number("index", index)
                .flag("default", position == 0)
                .flag("selected", position < 2)
                .text("displayTitle", titles[position])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::{json, library::World, scenario::Scenario};

    fn item() -> FakeItem {
        World::build(&Scenario::behaving(1)).libraries[0].items[0].clone()
    }

    #[test]
    fn a_part_says_nothing_about_the_file_unless_the_request_asked() {
        // `accessible: None` is Plex not having looked. Sent always, the case
        // a broken-media badge has to survive is unreachable (P1).
        let rendered = json::document(&media(&item(), Detail::PLAIN));
        let part = &rendered["Media"]["Part"][0];
        assert!(part.get("accessible").is_none());
        assert!(part.get("exists").is_none());
    }

    #[test]
    fn a_request_that_asked_for_a_file_check_is_told() {
        let rendered = json::document(&media(
            &item(),
            Detail {
                check_files: true,
                ..Detail::PLAIN
            },
        ));
        assert_eq!(rendered["Media"]["Part"][0]["accessible"], 1);
        assert_eq!(rendered["Media"]["Part"][0]["exists"], 1);
    }

    #[test]
    fn the_sometimes_reported_attributes_are_there_by_default() {
        let rendered = json::document(&media(&item(), Detail::PLAIN));
        assert_eq!(rendered["Media"]["videoProfile"], "high");
        assert_eq!(rendered["Media"]["videoFrameRate"], "24p");
        assert!(rendered["Media"]["aspectRatio"].is_number());
        assert_eq!(rendered["Media"]["Part"][0]["Stream"][0]["index"], 0);
        assert_eq!(
            rendered["Media"]["Part"][0]["Stream"][1]["displayTitle"],
            "English (EAC3 5.1)"
        );
    }

    #[test]
    fn a_scenario_can_withhold_every_one_of_them() {
        let rendered = json::document(&media(
            &item(),
            Detail {
                withhold: true,
                ..Detail::PLAIN
            },
        ));
        assert!(rendered["Media"].get("aspectRatio").is_none());
        assert!(rendered["Media"].get("videoProfile").is_none());
        let stream = &rendered["Media"]["Part"][0]["Stream"][0];
        assert!(stream.get("index").is_none());
        assert!(stream.get("selected").is_none());
        assert_eq!(stream["streamType"], 1, "the fact itself is still there");
    }
}
