"""Dress the original shareware maps and stock the final encounter."""
import math
import struct

LINE = struct.Struct('<7H')
SIDE = struct.Struct('<hh8s8s8sH')
SECTOR = struct.Struct('<hh8s8shhh')
POINT = struct.Struct('<hh')
THING = struct.Struct('<5h')


def apply(lumps):
    result = list(lumps)
    for number in range(1, 10):
        marker = f'E1M{number}'.encode()
        start = next(i for i, item in enumerate(lumps) if item[0] == marker)
        indices = {name: i for i, (name, _) in enumerate(lumps[start+1:start+11], start+1)}
        def data(name):
            return lumps[indices[name]][1]
        def put(name, data):
            result[indices[name]] = (name, bytes(data))

        sides = bytearray(data(b'SIDEDEFS'))
        vertices = list(POINT.iter_unpack(data(b'VERTEXES')))
        lines = list(LINE.iter_unpack(data(b'LINEDEFS')))
        things = bytearray(data(b'THINGS'))
        px, py, heading, _, _ = next(t for t in THING.iter_unpack(things) if t[3] == 1)
        # A small group of walls around the entrance carries foundry markings.
        # Use only plain, one-sided walls and keep doors, keyed switches and trim.
        plain = {b'STARTAN3', b'STARTAN2', b'BROWN96', b'BROWN1', b'BROWNHUG',
                 b'STONE2', b'STONE3', b'GRAY1', b'STONE', b'STARG3', b'STARG1'}
        candidates = []
        for a, b, _, action, _, right, left in lines:
            if left != 65535 or action:
                continue
            u, v = vertices[a], vertices[b]
            if math.dist(u, v) < 56:
                continue
            index = right * SIDE.size
            _, _, _, _, middle, _ = SIDE.unpack_from(sides, index)
            if middle.rstrip(b'\0').upper() not in plain:
                continue
            mx, my = (u[0]+v[0])/2, (u[1]+v[1])/2
            distance = math.hypot(mx-px, my-py)
            if distance < 750:
                candidates.append((distance, index))
        for rank, (_, offset) in enumerate(sorted(candidates)[:4]):
            material = b'BRNSMAL1' if rank == 0 else b'BRNSMALC'
            sides[offset+20:offset+28] = material.ljust(8, b'\0')
        if number == 1:
            # Early shotgun, shells, and a living middle shell across the blue floor.
            additions = [(1080,-3530,0,2001),(1098,-3500,0,2008),(1080,-3480,0,2008),
                         (1110,-2980,270,3001)]
        elif number == 8:
            # Two supply stations. The arena is winnable from a pistol start.
            additions = [(-88,-224,0,2003),(-60,-224,0,2046),(-40,-224,0,2046),
                         (-20,-224,0,2019),(0,-200,0,2002),(0,-248,0,2048),
                         (368,1480,0,2046),(464,1480,0,2046),(416,1550,0,2012),
                         (370,1850,0,2003),(462,1850,0,2046),(416,1870,0,2046)]
            # Make the backs of the monster alcoves barrel-press identification plates.
            for side_index in (251, 283):
                offset = SIDE.size * side_index
                struct.pack_into('<hh', sides, offset, 0, 0)
                sides[offset+20:offset+28] = b'BRNSMAL2'
            sectors = bytearray(data(b'SECTORS'))
            for s in (27, 28, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49):
                offset = s * SECTOR.size
                sectors[offset+4:offset+12] = b'FLOOR0_1'
            for s in (43, 44, 47, 48):
                struct.pack_into('<h', sectors, s*SECTOR.size+20, 240)
            put(b'SECTORS', sectors)
        else:
            additions = []
        for x, y, angle, kind in additions:
            things.extend(THING.pack(x,y,angle,kind,7))
        put(b'THINGS', things)
        put(b'SIDEDEFS', sides)
    return result
