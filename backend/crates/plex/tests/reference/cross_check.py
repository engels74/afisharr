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
    from plexapi.collection import Collection
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
    # The genre is read off the library for the same reason the year pivot
    # below is: this runs against a real server too, and a hard-coded `Comedy`
    # there is a filter that matches everything or nothing for a reason that is
    # not the fake's. A library with no genre that narrows is a named failure
    # rather than a check that quietly asserts nothing.
    carried: dict[str, int] = {}
    for item in everything:
        for genre in getattr(item, "genres", None) or []:
            carried[genre.tag] = carried.get(genre.tag, 0) + 1
    narrowing = sorted(tag for tag, count in carried.items() if 0 < count < len(everything))
    if report.expect(
        "some genre is carried by some but not all items, to narrow on",
        bool(narrowing),
    ):
        wanted = narrowing[0]
        matching = section.search(genre=wanted)
        report.expect(
            f"search(genre={wanted!r}) narrows the result",
            0 < len(matching) < len(everything),
        )
    # The pivot is read off the library rather than written down: this runs
    # against a real server too, and a hard-coded year there is a filter that
    # matches everything or nothing for a reason that is not the fake's. A
    # library where nothing carries a year is a named failure rather than an
    # IndexError, because a traceback says only that something went wrong.
    years = sorted(item.year for item in everything if item.year)
    if report.expect("some item carries a year to pivot a range filter on", bool(years)):
        pivot = years[len(years) // 2]
        modern = section.search(**{"year>>": pivot})
        report.expect(
            f"a range filter at {pivot} narrows the result",
            0 < len(modern) <= len(everything),
        )
        # Applied by the reference client itself, over the answer: it needs the
        # attribute to be on the row at all, which is a different claim about
        # the fake from the one above.
        recent = section.search(**{"year__gte": pivot})
        report.expect(
            f"a client-side comparison at {pivot} narrows the result",
            0 < len(recent) <= len(everything),
        )

    # Case-folded, and with the title standing in where the server sent no
    # sort title: Plex sorts case-insensitively and `sorted()` does not, so a
    # real library holding `apple` beside `Banana` would fail this for a
    # reason that is not the server's ordering. A `None` in the list would not
    # even compare.
    sorted_titles = [
        item.titleSort or item.title or "" for item in section.search(sort="titleSort:asc")
    ]
    report.expect(
        "search(sort=...) orders the result",
        sorted_titles == sorted(sorted_titles, key=str.lower),
    )
    windowed = section.search(container_start=1, container_size=2, maxresults=2)
    report.expect("a windowed search returns the window", len(windowed) == 2)

    as_collections = section.search(libtype="collection")
    report.require("search(libtype='collection') answers collections", as_collections)
    report.expect(
        "and answers collections rather than films",
        all(row.type == "collection" for row in as_collections),
    )


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
    # Read off the payload for the same reason the manage row's flags are: the
    # reference client defaults `collectionSort` to `0`
    # (`plexapi/collection.py:72`), so `is not None` passes on an answer that
    # never mentioned it -- and "release order" would then be a fact this
    # repository invented about somebody's collection (P1).
    report.expect(
        "the server states collectionSort",
        "collectionSort" in collection._data.attrib,
    )
    report.require("collection.items()", collection.items())
    return collection


def check_hubs(section, collection, report: Report):
    hubs = section.managedHubs()
    report.require("section.managedHubs() returns rows", hubs)
    for hub in hubs:
        report.require("hub.identifier", hub.identifier)
        report.require("hub.title", hub.title)
        # Read off the payload, not off the parsed object. The reference client
        # substitutes a default for every one of these when the attribute is
        # absent -- `deletable` defaults to True and the three axes to False
        # (`plexapi/library.py:3035-3040`) -- so `hub.deletable is not None`
        # can never fail and a fake that stopped sending the attribute
        # altogether would still read green here. `deletable` is what
        # `HubKind` is classified from and the axes are what §15.5 turns on, so
        # what has to be checked is that the *server* stated them.
        for stated in (
            "deletable",
            "promotedToOwnHome",
            "promotedToSharedHome",
            "promotedToRecommended",
        ):
            report.expect(
                f"the server states {stated} on {hub.identifier}",
                stated in hub._data.attrib,
            )
        report.require(f"hub.homeVisibility on {hub.identifier}", hub.homeVisibility)
        report.require(
            f"hub.recommendationsVisibility on {hub.identifier}",
            hub.recommendationsVisibility,
        )

    if collection is None:
        return
    visibility = collection.visibility()
    report.require("collection.visibility() returns a hub", visibility)
    report.require("the hub it returns names itself", visibility.identifier)


def check_writes(section, item, collection, report: Report):
    """The writes, every reversible one of them reversed before this returns.

    **Never run against a server this run must not touch.** `uploadPoster`
    below cannot be undone -- Plex selects an uploaded poster the moment it
    arrives and the one it replaced is not addressable from here -- so the
    real-server lane passes `--read-only` and never reaches this function
    (`tests/reference.rs`). Everything else is wrapped so a failure mid-way
    still unwinds to its own reversal, because a check that raises between a
    write and its undo leaves the library changed (P2).
    """
    before = {label.tag for label in item.labels}
    item.addLabel("afisharr-cross-check", locked=False)
    try:
        item.reload()
        report.expect(
            "editTags() adds a label the server reports back",
            "afisharr-cross-check" in {label.tag for label in item.labels},
        )
    finally:
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
        try:
            collection.reload()
            report.expect(
                "collection.edit() writes the title the server reports back",
                collection.title == f"{original} (cross-check)",
            )
        finally:
            collection.edit(**{"title.value": original})

    # A collection nothing has promoted is in the library and not in the
    # ordering space, and the reference client tells the two apart by getting
    # no row back and synthesising one with `_promoted` false
    # (`plexapi/collection.py:207-215`). Created and deleted here, because this
    # runs on somebody's real Plex (P2).
    fresh = Collection.create(
        section._server,
        "Afisharr cross-check (safe to delete)",
        section,
        items=[item],
    )
    try:
        report.expect(
            "a created collection is in the library",
            fresh.ratingKey is not None,
        )
        unpromoted = fresh.visibility()
        report.expect(
            "and not in the ordering space until something promotes it",
            unpromoted._promoted is False,
        )
        report.require(
            "the synthesised row still names the collection",
            unpromoted.identifier,
        )
        unpromoted.promoteHome()
        promoted = fresh.visibility()
        report.expect(
            "promoting it puts a real row in the manage list",
            promoted._promoted is True,
        )
        report.expect(
            "and the row it put there is on the home screen",
            promoted.promotedToOwnHome is True,
        )

        # A collection carries labels exactly as an item does — `LabelMixin` is
        # in `CollectionEditMixins` (`plexapi/mixins/__init__.py:115-120`) — and
        # `label` is the only filter the `collection` libtype declares. Checked
        # together, because a fake could hold the label and still hand back
        # every collection to the filter that asked for it.
        fresh.addLabel("afisharr-cross-check", locked=False)
        fresh.reload()
        report.expect(
            "a collection reports back the label it was given",
            "afisharr-cross-check" in {label.tag for label in fresh.labels},
        )
        labelled = section.search(libtype="collection", label="afisharr-cross-check")
        report.expect(
            "and a label filter answers that collection and no other",
            [row.ratingKey for row in labelled] == [fresh.ratingKey],
        )
        fresh.removeLabel("afisharr-cross-check", locked=False)
        fresh.reload()
        report.expect(
            "and removing it leaves the collection carrying none",
            not fresh.labels,
        )
    finally:
        fresh.delete()


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
