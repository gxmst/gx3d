# Generates assets/scenes/showcase.json: tornado + ocean + destructible
# building zones combined in one map. Re-run after tweaking layout numbers.
import json, io, math

scene = {
    "player": {
        "position": [0.0, 2.0, 46.0],
        "height": 1.7,
        "move_speed": 9.0,
        "mouse_sensitivity": 0.0021,
    },
    "meshes": [
        {"name": "cube", "source": "cube"},
        {"name": "sphere", "source": {"sphere": {"segments": 24, "rings": 12}}},
        {"name": "cylinder", "source": {"cylinder": {"segments": 24}}},
        {"name": "rifle", "source": "rifle"},
        {"name": "wedge", "source": "wedge"},
        {"name": "humanoid", "source": "humanoid"},
    ],
    "materials": [
        {"name": "ground_sand", "color": [0.52, 0.46, 0.34], "roughness": 0.95},
        {"name": "beach_sand", "color": [0.62, 0.55, 0.4], "roughness": 0.92},
        {"name": "seabed_sand", "color": [0.3, 0.3, 0.24], "roughness": 0.95},
        {"name": "shore_rock", "color": [0.34, 0.33, 0.3], "roughness": 0.9},
        {"name": "concrete", "color": [0.55, 0.55, 0.52], "roughness": 0.88},
        {"name": "brick", "color": [0.48, 0.24, 0.17], "roughness": 0.85},
        {"name": "box", "color": [0.58, 0.36, 0.18], "roughness": 0.78},
        {"name": "bouncy_rubber", "color": [0.82, 0.12, 0.16], "roughness": 0.88},
        {"name": "ice", "color": [0.36, 0.72, 0.96], "roughness": 0.16},
        {"name": "heavy_metal", "color": [0.22, 0.25, 0.28], "roughness": 0.32, "metallic": 0.92},
        {"name": "sea_water", "color": [0.03, 0.14, 0.22], "roughness": 0.08, "water": True},
        {"name": "wood_plank", "color": [0.42, 0.28, 0.15], "roughness": 0.82},
        {"name": "barrel_red", "color": [0.72, 0.14, 0.1], "roughness": 0.45, "metallic": 0.55},
        {"name": "dust", "color": [0.45, 0.42, 0.38], "roughness": 0.95},
        {"name": "pier_stone", "color": [0.4, 0.4, 0.42], "roughness": 0.9},
        {"name": "buoy_orange", "color": [0.95, 0.45, 0.08], "roughness": 0.5},
        {"name": "enemy_body", "color": [0.75, 0.72, 0.68], "roughness": 0.7},
        {"name": "sign_glow", "color": [0.9, 0.85, 0.6], "roughness": 0.4, "emissive": [1.6, 1.4, 0.7]},
    ],
    "lights": [
        {"type": "directional", "direction": [0.35, -1.0, 0.25], "color": [1.0, 0.96, 0.88], "intensity": 3.2},
        {"type": "point", "position": [-52.0, 8.0, -30.0], "color": [1.0, 0.75, 0.4], "intensity": 30.0, "range": 40.0},
        {"type": "point", "position": [55.0, 10.0, -35.0], "color": [0.5, 0.8, 1.0], "intensity": 26.0, "range": 45.0},
        {"type": "point", "position": [0.0, 6.0, 40.0], "color": [1.0, 0.9, 0.7], "intensity": 18.0, "range": 30.0},
    ],
    "entities": [],
    "tornado": {
        "center": [-55.0, 0.0, -35.0],
        "radius": 14.0,
        "height": 30.0,
        "strength": 300.0,
        "wander": 7.0,
    },
    "water": {
        "center": [95.0, 0.4, 0.0],
        "size": 120.0,
        "subdivisions": 128,
        "amplitude": 0.5,
        "material": "sea_water",
    },
    "structures": [
        {
            "position": [0.0, 0.0, -55.0],
            "block_half_extents": [0.55, 0.4, 0.55],
            "blocks": [6, 10, 6],
            "hollow": True,
            "block_mass": 40.0,
            "mesh": "cube",
            "material": "brick",
        },
        {
            "position": [14.0, 0.0, -52.0],
            "block_half_extents": [0.5, 0.35, 0.5],
            "blocks": [4, 6, 4],
            "hollow": False,
            "block_mass": 30.0,
            "mesh": "cube",
            "material": "concrete",
        },
    ],
}

ents = scene["entities"]


def ent(name, mesh, material, pos, scale=None, rot=None, physics=None, interaction=None):
    e = {"name": name, "mesh": mesh, "material": material, "transform": {"position": pos}}
    if scale:
        e["transform"]["scale"] = scale
    if rot:
        e["transform"]["rotation"] = rot
    if physics:
        e["physics"] = physics
    if interaction:
        e["interaction"] = interaction
    ents.append(e)


# Terrain: a carved bay instead of one flat plane. West mainland at y=0,
# seabed at y=-3 under the water (level 0.4), terraced beach in between,
# and mainland corners north/south of the bay mouth.
ent("ground_west", "cube", "ground_sand", [-42.5, -3.0, 0.0], scale=[155.0, 6.0, 240.0],
    physics={"shape": {"cuboid": [77.5, 3.0, 120.0]}, "mass": 0.0})
ent("seabed", "cube", "seabed_sand", [87.5, -6.0, 0.0], scale=[105.0, 6.0, 120.0],
    physics={"shape": {"cuboid": [52.5, 3.0, 60.0]}, "mass": 0.0})
ent("ground_ne", "cube", "ground_sand", [87.5, -3.0, 90.0], scale=[105.0, 6.0, 60.0],
    physics={"shape": {"cuboid": [52.5, 3.0, 30.0]}, "mass": 0.0})
ent("ground_se", "cube", "ground_sand", [87.5, -3.0, -90.0], scale=[105.0, 6.0, 60.0],
    physics={"shape": {"cuboid": [52.5, 3.0, 30.0]}, "mass": 0.0})
# Terraced beach: four wide steps from the shore down to the seabed.
for i in range(4):
    top = -0.75 * (i + 1)
    ent("beach_step_%d" % i, "cube", "beach_sand", [36.5 + i * 3.0, top - 3.0, 0.0],
        scale=[3.0, 6.0, 120.0],
        physics={"shape": {"cuboid": [1.5, 3.0, 60.0]}, "mass": 0.0})

# --- Tornado zone (west): debris field of light props for the funnel ---
for i in range(26):
    a = i * 2.399963  # golden angle spiral
    r = 4.0 + (i % 9) * 1.3
    x = -55.0 + math.cos(a) * r
    z = -35.0 + math.sin(a) * r
    kind = i % 3
    if kind == 0:
        ent("debris_box_%d" % i, "cube", "wood_plank", [round(x, 2), 0.45, round(z, 2)],
            scale=[0.7, 0.7, 0.7],
            physics={"shape": {"cuboid": [0.35, 0.35, 0.35]}, "mass": 3.0})
    elif kind == 1:
        ent("debris_ball_%d" % i, "sphere", "bouncy_rubber", [round(x, 2), 0.4, round(z, 2)],
            scale=[0.6, 0.6, 0.6],
            physics={"shape": {"sphere": 0.3}, "mass": 1.5, "surface": "rubber"})
    else:
        ent("debris_barrel_%d" % i, "cylinder", "barrel_red", [round(x, 2), 0.55, round(z, 2)],
            scale=[0.6, 1.1, 0.6],
            physics={"shape": {"cylinder": [0.3, 0.55]}, "mass": 5.0})

# A row of loose planks near the tornado path that gets shredded.
for i in range(6):
    ent("shed_plank_%d" % i, "cube", "wood_plank", [-42.0 + i * 0.4, 1.2 + i * 0.15, -22.0],
        scale=[0.3, 2.4, 1.6], rot=[0.0, 12.0 * i, 0.0],
        physics={"shape": {"cuboid": [0.15, 1.2, 0.8]}, "mass": 6.0})

# --- Ocean zone (east): pier + floating props ---
ent("pier_deck", "cube", "wood_plank", [38.0, 0.9, -38.0], scale=[24.0, 0.5, 5.0],
    physics={"shape": {"cuboid": [12.0, 0.25, 2.5]}, "mass": 0.0})
ent("pier_step", "wedge", "pier_stone", [24.5, 0.45, -38.0], scale=[3.0, 0.9, 5.0],
    rot=[0.0, 180.0, 0.0],
    physics={"shape": {"wedge": [1.5, 0.45, 2.5]}, "mass": 0.0})
# Pier piles: pairs of posts from the seabed up to the deck.
for i, px in enumerate([30.0, 38.0, 46.0]):
    for j, pz in enumerate([-40.0, -36.0]):
        ent("pier_pile_%d_%d" % (i, j), "cylinder", "pier_stone", [px, -1.1, pz],
            scale=[0.4, 4.4, 0.4],
            physics={"shape": {"cylinder": [0.2, 2.2]}, "mass": 0.0})
# Pier railing posts along the south edge.
for i in range(5):
    ent("pier_rail_%d" % i, "cube", "wood_plank", [28.0 + i * 5.0, 1.7, -40.3],
        scale=[0.12, 1.0, 0.12])
ent("pier_rail_beam", "cube", "wood_plank", [38.0, 2.15, -40.3], scale=[24.0, 0.1, 0.12])
# Shoreline rocks scattered along the waterline for scale reference.
import random
rng = random.Random(7)
for i in range(9):
    rz = -55.0 + i * 13.0 + rng.uniform(-3.0, 3.0)
    rx = 34.0 + rng.uniform(-1.5, 4.0)
    ry = rng.uniform(-0.6, 0.1)
    sc = rng.uniform(0.8, 2.2)
    ent("shore_rock_%d" % i, "wedge", "shore_rock", [round(rx, 2), round(ry, 2), round(rz, 2)],
        scale=[round(sc, 2), round(sc * 0.7, 2), round(sc * 1.3, 2)],
        rot=[0.0, round(rng.uniform(0.0, 360.0), 1), 0.0],
        physics={"shape": {"wedge": [round(sc / 2, 2), round(sc * 0.35, 2), round(sc * 0.65, 2)]}, "mass": 0.0})
for i, (dx, dz) in enumerate([(-6.0, -2.0), (-2.5, 1.8), (1.0, -1.0), (4.5, 2.2)]):
    ent("float_crate_%d" % i, "cube", "wood_plank",
        [48.0 + dx * 2.0, 1.6, -38.0 + dz * 3.0], scale=[1.1, 1.1, 1.1],
        physics={"shape": {"cuboid": [0.55, 0.55, 0.55]}, "mass": 6.0})
for i in range(3):
    ent("buoy_%d" % i, "sphere", "buoy_orange", [58.0 + i * 8.0, 1.4, -28.0 - i * 6.0],
        scale=[0.9, 0.9, 0.9],
        physics={"shape": {"sphere": 0.45}, "mass": 3.0})
ent("float_barrel", "cylinder", "barrel_red", [52.0, 1.6, -46.0], scale=[0.7, 1.2, 0.7],
    physics={"shape": {"cylinder": [0.35, 0.6]}, "mass": 8.0})
# One heavy block that should sink.
ent("sink_block", "cube", "heavy_metal", [55.0, 1.5, -33.0], scale=[0.9, 0.9, 0.9],
    physics={"shape": {"cuboid": [0.45, 0.45, 0.45]}, "mass": 900.0})

# --- Building zone (north-center): explosive barrels around the towers ---
for i, (dx, dz) in enumerate([(-4.5, 4.0), (4.5, 3.5), (0.0, -5.5), (8.0, -2.0)]):
    ent("boom_barrel_%d" % i, "cylinder", "barrel_red", [0.0 + dx, 0.8, -55.0 + dz],
        scale=[0.7, 1.4, 0.7],
        physics={"shape": {"cylinder": [0.35, 0.7]}, "mass": 10.0},
        interaction={"type": "explosive", "radius": 7.0, "impulse": 30.0})

# Spawn-area sign and a few sandbox props near the player start.
ent("welcome_sign", "cube", "sign_glow", [0.0, 2.6, 38.0], scale=[6.0, 1.2, 0.3])
ent("sign_post_l", "cylinder", "heavy_metal", [-2.6, 1.3, 38.0], scale=[0.15, 2.6, 0.15],
    physics={"shape": {"cylinder": [0.08, 1.3]}, "mass": 0.0})
ent("sign_post_r", "cylinder", "heavy_metal", [2.6, 1.3, 38.0], scale=[0.15, 2.6, 0.15],
    physics={"shape": {"cylinder": [0.08, 1.3]}, "mass": 0.0})
for i in range(4):
    ent("start_box_%d" % i, "cube", "box", [-3.0 + i * 2.0, 0.5, 30.0],
        physics={"shape": {"cuboid": [0.5, 0.5, 0.5]}, "mass": 4.0})

scene["enemies"] = {
    "mesh": "humanoid",
    "material": "enemy_body",
    "spawn_points": [[-20.0, 1.0, -20.0], [20.0, 1.0, -15.0]],
    "waypoints": [[-20.0, 1.0, -20.0], [0.0, 1.0, -30.0], [20.0, 1.0, -15.0], [0.0, 1.0, 0.0]],
    "routes": [],
}

io.open("assets/scenes/showcase.json", "w", encoding="utf-8", newline="\n").write(
    json.dumps(scene, indent=2, ensure_ascii=False)
)
print("entities:", len(ents))
