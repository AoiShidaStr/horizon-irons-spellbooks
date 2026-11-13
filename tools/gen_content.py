#!/usr/bin/env python3
"""Writes the content mod's data, item models and original placeholder art.

Run from the mod folder: python tools/gen_content.py

The art is drawn here, procedurally, in 16x16: nothing is copied from the
original mod (All-Rights-Reserved). Every file it writes is generated, so the
only hand-kept sources are this script and the mod's Rust code.
"""

import json
import os
import struct
import zlib

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "content")
NS = "irons_spellbooks"

SCHOOLS = {
    "firebolt": (0xF0, 0x7A, 0x1E),
    "fireball": (0xF0, 0x4A, 0x1E),
    "icicle": (0x8F, 0xE3, 0xF5),
    "frost_step": (0x6F, 0xC3, 0xE5),
    "lightning_bolt": (0x5C, 0x7C, 0xFF),
    "heal": (0xF7, 0xE8, 0x8A),
    "fortify": (0xE7, 0xD0, 0x6A),
    "magic_missile": (0xC0, 0x5C, 0xF0),
    "teleport": (0x9A, 0x3C, 0xD0),
    "gust": (0x7B, 0xE0, 0x8C),
}

SPELL_NAMES = {
    "firebolt": "Firebolt",
    "fireball": "Fireball",
    "icicle": "Icicle",
    "frost_step": "Frost Step",
    "lightning_bolt": "Lightning Bolt",
    "heal": "Heal",
    "fortify": "Fortify",
    "magic_missile": "Magic Missile",
    "teleport": "Teleport",
    "gust": "Gust",
}

SPELL_FR = {
    "firebolt": "Trait de feu",
    "fireball": "Boule de feu",
    "icicle": "Stalactite",
    "frost_step": "Pas de givre",
    "lightning_bolt": "Éclair",
    "heal": "Soin",
    "fortify": "Renfort",
    "magic_missile": "Projectile magique",
    "teleport": "Téléportation",
    "gust": "Bourrasque",
}

BOOKS = {
    "apprentice_spellbook": ("Apprentice Spellbook", "Grimoire d'apprenti", "uncommon", 5),
    "journeyman_spellbook": ("Journeyman Spellbook", "Grimoire de compagnon", "rare", 8),
    "archmage_spellbook": ("Archmage Spellbook", "Grimoire d'archimage", "epic", 10),
}

ARMOUR = {
    "wizard_hat": ("Wizard Hat", "Chapeau de sorcier", "minecraft:leather_helmet"),
    "wizard_robe": ("Wizard Robe", "Robe de sorcier", "minecraft:leather_chestplate"),
    "wizard_leggings": ("Wizard Leggings", "Jambières de sorcier", "minecraft:leather_leggings"),
    "wizard_boots": ("Wizard Boots", "Bottes de sorcier", "minecraft:leather_boots"),
}


def write(path, text):
    full = os.path.join(ROOT, path)
    os.makedirs(os.path.dirname(full), exist_ok=True)
    with open(full, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(text)


def write_json(path, data):
    write(path, json.dumps(data, indent=2, ensure_ascii=False) + "\n")


def png(path, pixels):
    """pixels: 16 rows of 16 (r, g, b, a) tuples."""
    raw = b"".join(
        b"\x00" + b"".join(bytes(pixel) for pixel in row) for row in pixels
    )

    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

    blob = b"\x89PNG\r\n\x1a\n"
    blob += chunk(b"IHDR", struct.pack(">IIBBBBB", 16, 16, 8, 6, 0, 0, 0))
    blob += chunk(b"IDAT", zlib.compress(raw, 9))
    blob += chunk(b"IEND", b"")
    full = os.path.join(ROOT, path)
    os.makedirs(os.path.dirname(full), exist_ok=True)
    with open(full, "wb") as handle:
        handle.write(blob)


def blank():
    return [[(0, 0, 0, 0) for _ in range(16)] for _ in range(16)]


def rect(pixels, x0, y0, x1, y1, colour):
    for y in range(y0, y1):
        for x in range(x0, x1):
            if 0 <= x < 16 and 0 <= y < 16:
                pixels[y][x] = colour


def shade(colour, factor):
    return (
        max(0, min(255, int(colour[0] * factor))),
        max(0, min(255, int(colour[1] * factor))),
        max(0, min(255, int(colour[2] * factor))),
        colour[3],
    )


PARCHMENT = (0xE8, 0xD9, 0xA8, 0xFF)
PARCHMENT_DARK = (0xBF, 0xA8, 0x72, 0xFF)
WOOD = (0x8B, 0x5A, 0x2B, 0xFF)
WOOD_DARK = (0x5E, 0x3B, 0x18, 0xFF)
PURPLE = (0x4B, 0x2E, 0x6B, 0xFF)
PURPLE_LIGHT = (0x6E, 0x49, 0x96, 0xFF)
GOLD = (0xE8, 0xC4, 0x4A, 0xFF)


def scroll(colour):
    pixels = blank()
    rect(pixels, 3, 2, 13, 14, PARCHMENT)
    rect(pixels, 3, 2, 13, 3, PARCHMENT_DARK)
    rect(pixels, 3, 13, 13, 14, PARCHMENT_DARK)
    rect(pixels, 2, 2, 3, 14, WOOD)
    rect(pixels, 13, 2, 14, 14, WOOD)
    rect(pixels, 2, 1, 14, 2, WOOD_DARK)
    rect(pixels, 2, 14, 14, 15, WOOD_DARK)
    # A rune of the school on the parchment.
    rect(pixels, 7, 5, 9, 11, colour)
    rect(pixels, 6, 7, 10, 9, colour)
    rect(pixels, 5, 4, 6, 5, colour)
    rect(pixels, 10, 10, 11, 11, colour)
    return pixels


def book(cover):
    pixels = blank()
    rect(pixels, 3, 2, 13, 14, cover)
    rect(pixels, 3, 2, 13, 3, shade(cover, 1.3))
    rect(pixels, 12, 3, 13, 14, shade(cover, 0.65))
    rect(pixels, 4, 3, 12, 4, PARCHMENT)
    rect(pixels, 5, 6, 11, 7, GOLD)
    rect(pixels, 5, 7, 6, 12, GOLD)
    rect(pixels, 10, 7, 11, 12, GOLD)
    rect(pixels, 5, 11, 11, 12, GOLD)
    return pixels


def hat():
    pixels = blank()
    for y in range(2, 11):
        half = 1 + (y - 2) * 2 // 3
        rect(pixels, 8 - half, y, 8 + half, y + 1, PURPLE)
    rect(pixels, 3, 11, 14, 13, PURPLE_LIGHT)
    rect(pixels, 3, 12, 14, 13, shade(PURPLE, 0.7))
    rect(pixels, 6, 10, 11, 11, GOLD)
    rect(pixels, 7, 3, 9, 4, GOLD)
    return pixels


def robe():
    pixels = blank()
    rect(pixels, 4, 3, 12, 6, PURPLE)
    rect(pixels, 2, 6, 14, 14, PURPLE)
    rect(pixels, 2, 6, 4, 14, PURPLE_LIGHT)
    rect(pixels, 12, 6, 14, 14, shade(PURPLE, 0.7))
    rect(pixels, 6, 6, 10, 8, GOLD)
    rect(pixels, 7, 8, 9, 14, GOLD)
    rect(pixels, 6, 2, 10, 3, PURPLE_LIGHT)
    return pixels


def leggings():
    pixels = blank()
    rect(pixels, 4, 2, 12, 5, PURPLE)
    rect(pixels, 4, 5, 7, 14, PURPLE)
    rect(pixels, 9, 5, 12, 14, PURPLE)
    rect(pixels, 4, 13, 7, 14, PURPLE_LIGHT)
    rect(pixels, 9, 13, 12, 14, PURPLE_LIGHT)
    rect(pixels, 4, 2, 12, 3, GOLD)
    return pixels


def boots():
    pixels = blank()
    rect(pixels, 3, 6, 7, 13, PURPLE)
    rect(pixels, 9, 6, 13, 13, PURPLE)
    rect(pixels, 2, 12, 8, 14, PURPLE_LIGHT)
    rect(pixels, 8, 12, 14, 14, PURPLE_LIGHT)
    rect(pixels, 3, 6, 7, 7, GOLD)
    rect(pixels, 9, 6, 13, 7, GOLD)
    return pixels


def staff():
    pixels = blank()
    for i in range(3, 12):
        rect(pixels, i, 15 - i, i + 1, 16 - i, WOOD if i % 2 else WOOD_DARK)
    rect(pixels, 11, 2, 14, 5, (0x9A, 0x6C, 0xF0, 0xFF))
    rect(pixels, 12, 3, 13, 4, (0xE0, 0xD0, 0xFF, 0xFF))
    rect(pixels, 12, 1, 13, 2, GOLD)
    return pixels


def essence():
    pixels = blank()
    rect(pixels, 6, 2, 10, 14, (0x9A, 0x6C, 0xF0, 0xFF))
    rect(pixels, 4, 5, 12, 11, (0x9A, 0x6C, 0xF0, 0xFF))
    rect(pixels, 6, 4, 10, 12, (0xC0, 0x9C, 0xFF, 0xFF))
    rect(pixels, 7, 6, 9, 10, (0xF0, 0xE8, 0xFF, 0xFF))
    rect(pixels, 2, 7, 4, 9, (0x9A, 0x6C, 0xF0, 0xFF))
    return pixels


def item_file(name, template, extra=None):
    data = {"template": template}
    if extra:
        data.update(extra)
    write_json(f"data/{NS}/horizon/item/{name}.json", data)
    write_json(
        f"assets/{NS}/items/{name}.json",
        {"model": {"type": "minecraft:model", "model": f"{NS}:item/{name}"}},
    )
    write_json(
        f"assets/{NS}/models/item/{name}.json",
        {"parent": "minecraft:item/generated", "textures": {"layer0": f"{NS}:item/{name}"}},
    )


def recipe(name, ingredients, result, count=1):
    write_json(
        f"data/{NS}/recipe/{name}.json",
        {
            "type": "minecraft:crafting_shapeless",
            "ingredients": ingredients,
            "result": {"id": result, "count": count},
        },
    )


def main():
    en = {}
    fr = {}
    tab = []

    # Scrolls: one item per spell, the school's colour on parchment.
    for spell, colour in SCHOOLS.items():
        name = f"scroll_{spell}"
        item_file(name, "minecraft:paper", {"max_stack_size": 16, "rarity": "uncommon"})
        png(f"assets/{NS}/textures/item/{name}.png", scroll(colour))
        en[f"item.{NS}.{name}"] = f"Scroll of {SPELL_NAMES[spell]}"
        fr[f"item.{NS}.{name}"] = f"Parchemin : {SPELL_FR[spell]}"
        tab.append(f"{NS}:{name}")

    # Spellbooks.
    for name, (english, french, rarity, _) in BOOKS.items():
        item_file(name, "minecraft:book", {"max_stack_size": 1, "rarity": rarity})
        png(f"assets/{NS}/textures/item/{name}.png", book(PURPLE))
        en[f"item.{NS}.{name}"] = english
        fr[f"item.{NS}.{name}"] = french
        tab.append(f"{NS}:{name}")

    # Mage armour.
    art = {
        "wizard_hat": hat,
        "wizard_robe": robe,
        "wizard_leggings": leggings,
        "wizard_boots": boots,
    }
    for name, (english, french, template) in ARMOUR.items():
        item_file(name, template, {"rarity": "uncommon"})
        png(f"assets/{NS}/textures/item/{name}.png", art[name]())
        en[f"item.{NS}.{name}"] = english
        fr[f"item.{NS}.{name}"] = french
        tab.append(f"{NS}:{name}")

    item_file("arcane_staff", "minecraft:blaze_rod", {"max_stack_size": 1, "rarity": "rare"})
    png(f"assets/{NS}/textures/item/arcane_staff.png", staff())
    en[f"item.{NS}.arcane_staff"] = "Arcane Staff"
    fr[f"item.{NS}.arcane_staff"] = "Bâton arcanique"
    tab.append(f"{NS}:arcane_staff")

    item_file("arcane_essence", "minecraft:amethyst_shard", {"max_stack_size": 64})
    png(f"assets/{NS}/textures/item/arcane_essence.png", essence())
    en[f"item.{NS}.arcane_essence"] = "Arcane Essence"
    fr[f"item.{NS}.arcane_essence"] = "Essence arcanique"
    tab.append(f"{NS}:arcane_essence")

    en[f"itemGroup.{NS}.spellbooks"] = "Iron's Spellbooks"
    fr[f"itemGroup.{NS}.spellbooks"] = "Iron's Spellbooks"

    write_json(f"assets/{NS}/lang/en_us.json", dict(sorted(en.items())))
    write_json(f"assets/{NS}/lang/fr_fr.json", dict(sorted(fr.items())))
    write_json(
        f"data/{NS}/horizon/creative_tab/spellbooks.json",
        {"title": f"itemGroup.{NS}.spellbooks", "icon": f"{NS}:archmage_spellbook", "items": tab},
    )

    # Recipes: essence first, then every scroll from paper + essence + a
    # school ingredient, the books from their lesser book, the armour from
    # leather, and the staff.
    recipe("arcane_essence", ["minecraft:amethyst_shard", "minecraft:glowstone_dust"],
           f"{NS}:arcane_essence", 2)

    school_ingredient = {
        "firebolt": ("minecraft:blaze_powder", 1),
        "fireball": ("minecraft:blaze_powder", 2),
        "icicle": ("minecraft:snowball", 1),
        "frost_step": ("minecraft:snowball", 2),
        "lightning_bolt": ("minecraft:lightning_rod", 1),
        "heal": ("minecraft:glowstone_dust", 1),
        "fortify": ("minecraft:glowstone_dust", 2),
        "magic_missile": ("minecraft:ender_pearl", 1),
        "teleport": ("minecraft:ender_pearl", 2),
        "gust": ("minecraft:feather", 1),
    }
    for spell, colour in SCHOOLS.items():
        ingredient, copies = school_ingredient[spell]
        ingredients = ["minecraft:paper", f"{NS}:arcane_essence", ingredient]
        ingredients += [f"{NS}:arcane_essence"] * (copies - 1)
        recipe(f"scroll_{spell}", ingredients, f"{NS}:scroll_{spell}")

    recipe("journeyman_spellbook",
           [f"{NS}:apprentice_spellbook"] + [f"{NS}:arcane_essence"] * 4,
           f"{NS}:journeyman_spellbook")
    recipe("archmage_spellbook",
           [f"{NS}:journeyman_spellbook"] + [f"{NS}:arcane_essence"] * 6,
           f"{NS}:archmage_spellbook")
    recipe("apprentice_spellbook",
           ["minecraft:book", "minecraft:leather"] + [f"{NS}:arcane_essence"] * 2,
           f"{NS}:apprentice_spellbook")
    for name, (_, _, template) in ARMOUR.items():
        recipe(name, [template, f"{NS}:arcane_essence", f"{NS}:arcane_essence"], f"{NS}:{name}")
    recipe("arcane_staff",
           ["minecraft:stick", "minecraft:blaze_rod"] + [f"{NS}:arcane_essence"] * 3,
           f"{NS}:arcane_staff")


if __name__ == "__main__":
    main()
    print("content written")
