// A resolution ladder for the same dense mesh. Each successful step presents both pages.
fn unmap(addr: isize, bytes: usize) {
    if addr >= 0 {
        call(11, addr as usize, bytes, 0, 0, 0, 0);
    }
}
fn resolution_probe(fb: isize, cache: &mut [Cached]) {
    for (w, h) in [
        (640, 360),
        (1280, 720),
        (1920, 1080),
        (2560, 1440),
        (2560, 1600),
        (3840, 2160),
    ] {
        let mut d = [w as u32, h as u32, 0, 0, 0, 0];
        metric(b"HMODE width=", w as u64);
        metric(b" height=", h as u64);
        if ioctl(fb, 0x5002, d.as_mut_ptr() as usize) != 0
            || d[0] != w as u32
            || d[1] != h as u32
            || d[2] != w as u32 * 4
            || d[3] != 2
        {
            log(b" rejected\n");
            continue;
        }
        let page = (w * h) as usize;
        let bytes = page * 4;
        let local = call(9, 0, bytes * 2, 3, 0x22, usize::MAX, 0);
        let dev = call(9, 0, bytes * 2, 3, 1, fb as usize, 0);
        if local < 0 || dev < 0 {
            unmap(local, bytes * 2);
            unmap(dev, bytes * 2);
            log(b" mapping_error\n");
            continue;
        }
        log(b" accepted\n");
        let px = unsafe { slice::from_raw_parts_mut(local as *mut u32, page) };
        let z = unsafe { slice::from_raw_parts_mut((local as *mut i32).add(page), page) };
        let mut samples = [(Counts::default(), 0u64, 0u64); 12];
        for i in 0..12 {
            let t = us();
            px.fill(0x18232b);
            z.fill(0);
            let mut count = Counts::default();
            Target::new(px, z, w, h, 444 * h / 360).draw(
                &LODS[2],
                heliograph::raster::Instance {
                    pos: [0, 0, 5300],
                    yaw: 426 + i * 5,
                    tilt: 76,
                    scale: 1010,
                },
                &heliograph::assets::MATERIALS[0],
                cache,
                &mut count,
            );
            let draw = us();
            let next = 1 - d[4].min(1);
            let out = unsafe {
                slice::from_raw_parts_mut((dev as *mut u32).add(next as usize * page), page)
            };
            out.copy_from_slice(px);
            if ioctl(fb, 0x5003, next as usize) != 0 {
                log(b"HELIOGRAPH probe flip error\n");
                exit(2);
            }
            d[4] = next;
            let end = us();
            log(b"F\n");
            samples[i as usize] = (count, draw - t, end - draw);
        }
        for (i, (count, r, p)) in samples.iter().copied().enumerate() {
            metric(b"HRES width=", w as u64);
            metric(b" height=", h as u64);
            metric(b" frame=", i as u64);
            metric(b" tris=", count.submitted as u64);
            metric(b" sampled=", count.sampled as u64);
            metric(b" render_us=", r);
            metric(b" present_us=", p);
            metric(b" total_us=", r + p);
            log(b"\n");
        }
        metric(b"HOLD width=", w as u64);
        metric(b" height=", h as u64);
        log(b"\n");
        let t = [1i64, 0];
        call(35, t.as_ptr() as usize, 0, 0, 0, 0, 0);
        unmap(local, bytes * 2);
        unmap(dev, bytes * 2);
    }
    let mut d = [W as u32, H as u32, 0, 0, 0, 0];
    if ioctl(fb, 0x5002, d.as_mut_ptr() as usize) != 0 {
        log(b"HELIOGRAPH restore error\n");
        exit(2);
    }
    log(b"HELIOGRAPH resolution ladder done\n");
}

