#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Afisharr contributors
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Read a Plex server with a client that was not written in this repository.

Every shape in the adversarial fake is a claim about a server nobody here
controls, and until this ran, every one of those claims was checked by the same
judgement that made it: the fake and this repository's client were written
together, against each other, from one reading of the protocol.

`python-plexapi` is a second reader. It was written against real servers by
people who were not in this room, it parses XML -- which is what a real Plex
answers by default and what the fake had never emitted -- and it fails on a
wrong attribute name whether or not anybody here suspected that name.

It is used as *evidence*, so the version is pinned (see `requirements.txt`).
An unpinned reference makes a lane that goes red for a reason unrelated to this
repository.

Run it against the fake or against a real server; it asks the same questions of
both. A `None` where a value belongs is the failure this exists to see, so
every check names the attribute it found empty rather than reporting that
something went wrong.
"""

from __future__ import annotations

import argparse
import json
import sys
import tempfile
from pathlib import Path

try:
    import plexapi
    from plexapi.server import PlexServer
except ImportError as missing:  # pragma: no cover - reported, never swallowed
    print(
        "The reference client is not installed. Install the pinned version from "
        "tests/reference/requirements.txt; a cross-check that quietly does not "
        "run reads green on the lane that was supposed to catch the drift.",
        file=sys.stderr,
    )
    raise SystemExit(2) from missing


class Report:
    """What the run saw, and everything it could not read."""

    def __init__(self) -> None:
        self.checks: list[str] = []
        self.failures: list[str] = []

    def ok(self, what: str) -> None:
        self.checks.append(what)

    def require(self, what: str, value: object) -> bool:
        """A value that must be there. `None` and empty are both failures."""
        if value is None or value == "" or value == []:
            self.failures.append(f"{what} came back {value!r}")
            return False
        self.checks.append(what)
        return True

    def expect(self, what: str, condition: bool) -> bool:
        if not condition:
            self.failures.append(what)
            return False
        self.checks.append(what)
        return True


def check_sections(server: PlexServer, report: Report):
    sections = server.library.sections()
    report.require("library.sections() returns sections", sections)
    movies = next((s for s in sections if s.type == "movie"), None)
    report.require("a movie section is readable", movies)
    if movies is None:
        return None
    for attribute in ("key", "uuid", "title", "agent", "scanner", "type"):
        report.require(f"section.{attribute}", getattr(movies, attribute, None))
    report.require("section.locations", movies.locations)
    report.require("section.createdAt", movies.createdAt)
    report.require("section.updatedAt", movies.updatedAt)
    return movies


def check_items(section, report: Report):
    items = section.search(maxresults=5)
    report.require("section.search() returns items", items)
    if not items:
        return None
    item = items[0]
    for attribute in ("ratingKey", "guid", "title", "key", "type", "addedAt"):
        report.require(f"item.{attribute}", getattr(item, attribute, None))
    report.expect(
        "item.librarySectionID resolves to the section it came from",
        item.librarySectionID == section.key,
    )
    report.require("item.guids", getattr(item, "guids", None))
    report.require("item.media", item.media)
    media = item.media[0]
    for attribute in ("container", "videoCodec", "videoResolution", "aspectRatio"):
        report.require(f"media.{attribute}", getattr(media, attribute, None))
    report.require("media.parts", media.parts)
    part = media.parts[0]
    report.require("part.file", part.file)
    report.require("part.streams", part.streams)
    stream = part.streams[0]
    for attribute in ("codec", "displayTitle"):
        report.require(f"stream.{attribute}", getattr(stream, attribute, None))
    return item


def check_filters(section, report: Report):
    filters = section.listFilters()
    report.require("section.listFilters() returns filters", filters)
    genre = next((f for f in filters if f.filter == "genre"), None)
    if report.require("a genre filter is declared", genre):
        report.require("filter.key names the endpoint its choices come from", genre.key)
        choices = section.listFilterChoices("genre")
        report.require("listFilterChoices('genre') returns choices", choices)
        for choice in choices:
            report.require("choice.key", choice.key)
            report.require("choice.title", choice.title)

    collection_filters = section.listFilters("collection")
    report.require(
        "listFilters('collection') returns the collection libtype's filters",
        collection_filters,
    )

    fields = section.listFields()
    report.require("section.listFields() returns fields", fields)
    # The reference client adds manual fields of its own, undotted, so the
    # claim is about the ones the *server* declared: it spells them
    # `{libtype}.{field}`, and a client sends the key straight back as the
    # filter argument (`plexapi/library.py:1082`).
    declared = {field.key for field in fields}
    for field in ("genre", "year", "title"):
        report.expect(
            f"the server declares {section.type}.{field}",
            f"{section.type}.{field}" in declared,
        )
    report.require(
        "the operator table answers for a tag field",
        section.listOperators("tag"),
    )


def check_search_arguments(section, report: Report):
    everything = section.search()
    comedies = section.search(genre="Comedy")
    report.expect(
        "search(genre=...) narrows the result",
        0 < len(comedies) < len(everything),
    )
    # The pivot is read off the library rather than written down: this runs
    # against a real server too, and a hard-coded year there is a filter that
    # matches everything or nothing for a reason that is not the fake's.
    years = sorted(item.year for item in everything if item.year)
    pivot = years[len(years) // 2]
    modern = section.search(**{"year>>": pivot})
    report.expect(
        f"a range filter at {pivot} narrows the result",
        0 < len(modern) <= len(everything),
    )
    sorted_titles = [item.titleSort for item in section.search(sort="titleSort:asc")]
    report.expect(
        "search(sort=...) orders the result",
        sorted_titles == sorted(sorted_titles),
    )
    windowed = section.search(container_start=1, container_size=2, maxresults=2)
    report.expect("a windowed search returns the window", len(windowed) == 2)


def check_collections(section, report: Report):
    collections = section.collections()
    report.require("section.collections() returns collections", collections)
    if not collections:
        return None
    collection = collections[0]
    for attribute in ("ratingKey", "key", "title", "subtype", "type"):
        report.require(f"collection.{attribute}", getattr(collection, attribute, None))
    report.expect(
        "collection.librarySectionID resolves to its own section",
        collection.librarySectionID == section.key,
    )
    report.expect(
        "collection.collectionSort is readable",
        collection.collectionSort is not None,
    )
    report.require("collection.items()", collection.items())
    return collection


def check_hubs(section, collection, report: Report):
    hubs = section.managedHubs()
    report.require("section.managedHubs() returns rows", hubs)
    for hub in hubs:
        report.require("hub.identifier", hub.identifier)
        report.require("hub.title", hub.title)
        report.expect(
            f"hub.deletable is readable on {hub.identifier}",
            hub.deletable is not None,
        )
        report.require(f"hub.homeVisibility on {hub.identifier}", hub.homeVisibility)
        report.require(
            f"hub.recommendationsVisibility on {hub.identifier}",
            hub.recommendationsVisibility,
        )
        for axis in (
            "promotedToOwnHome",
            "promotedToSharedHome",
            "promotedToRecommended",
        ):
            report.expect(
                f"hub.{axis} is readable on {hub.identifier}",
                getattr(hub, axis, None) is not None,
            )

    if collection is None:
        return
    visibility = collection.visibility()
    report.require("collection.visibility() returns a hub", visibility)
    report.require("the hub it returns names itself", visibility.identifier)


def check_writes(section, item, collection, report: Report):
    """The writes, all of them reversed before this returns.

    Run against somebody's real Plex as well as against the fake, so nothing
    here may be left behind (P2).
    """
    before = {label.tag for label in item.labels}
    item.addLabel("afisharr-cross-check", locked=False)
    item.reload()
    report.expect(
        "editTags() adds a label the server reports back",
        "afisharr-cross-check" in {label.tag for label in item.labels},
    )
    item.removeLabel("afisharr-cross-check", locked=False)
    item.reload()
    report.expect(
        "editTags() removes it again, leaving what was there",
        {label.tag for label in item.labels} == before,
    )

    with tempfile.TemporaryDirectory() as directory:
        poster = Path(directory) / "poster.png"
        # A one-pixel PNG. The bytes matter only in that they are bytes.
        poster.write_bytes(
            bytes.fromhex(
                "89504e470d0a1a0a0000000d49484452000000010000000108060000001f15c4"
                "890000000a49444154789c6360000002000100ffff03000006000557bfabd400"
                "00000049454e44ae426082"
            )
        )
        item.uploadPoster(filepath=str(poster))
        report.ok("uploadPoster() is accepted")

    if collection is not None:
        original = collection.title
        collection.edit(**{"title.value": f"{original} (cross-check)"})
        collection.reload()
        report.expect(
            "collection.edit() writes the title the server reports back",
            collection.title == f"{original} (cross-check)",
        )
        collection.edit(**{"title.value": original})


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-url", required=True)
    parser.add_argument("--token", required=True)
    parser.add_argument(
        "--read-only",
        action="store_true",
        help="skip the writes, for a server this run must not touch",
    )
    arguments = parser.parse_args()

    report = Report()
    server = PlexServer(arguments.base_url, arguments.token)
    report.require("the server names itself", server.machineIdentifier)
    report.require("the server names its version", server.version)

    section = check_sections(server, report)
    if section is not None:
        item = check_items(section, report)
        check_filters(section, report)
        check_search_arguments(section, report)
        collection = check_collections(section, report)
        check_hubs(section, collection, report)
        if not arguments.read_only and item is not None:
            check_writes(section, item, collection, report)

    print(
        json.dumps(
            {
                "reference": plexapi.__version__,
                "checks": len(report.checks),
                "failures": report.failures,
            },
            indent=2,
        )
    )
    if report.failures:
        print(
            "\n".join(
                ["The reference client could not read:"]
                + [f"  - {failure}" for failure in report.failures]
            ),
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
