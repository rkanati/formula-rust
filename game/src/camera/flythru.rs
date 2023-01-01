use {
    ultraviolet as uv,
};

pub struct FlythruCam {
    pub(crate) points: Vec<uv::Vec3>,
    t: f32,
}

impl FlythruCam {
    pub fn new(graph: &[bundle::ArchivedTrackNode]) -> Self {
        let mut points = Vec::new();
        let mut node_i = 0;

        loop {
            let s = &graph[node_i as usize];
            points.push(s.center.into());
            let next = Some(s.next[0]).filter(|&i| i != !0u32);
            let junc = Some(s.next[1]).filter(|&i| i != !0u32);
            node_i = next.or(junc).unwrap_or(0);
            if node_i == 0 {break}
        }

        loop {
            let s = &graph[node_i as usize];
            points.push(s.center.into());
            let next = Some(s.next[0]).filter(|&i| i != !0u32);
            let junc = Some(s.next[1]).filter(|&i| i != !0u32);
            node_i = junc.or(next).unwrap_or(0);
            if node_i == 0 {break}
        }

        Self{points, t: 0.}
    }

    fn eval(&self, t: f32) -> uv::Vec3 {
        let t = t.fract();

        let u = self.points.len() as f32 * t;
        let v = u.floor();
        let i0 = u as i32;
        let t = u - v;

        let [p0, p1, p2, p3]: [uv::Vec3; 4] = [i0-1, i0, i0+1, i0+2]
            .map(|i| self.points[i.rem_euclid(self.points.len() as i32) as usize]);

        let m0 = 0.5 * (p2 - p0);
        let m1 = 0.5 * (p3 - p1);

        let t2 = t * t;
        let t3 = t2 * t;

        let h0 =  2. * t3 - 3. * t2     + 1.;
        let h1 =       t3 - 2. * t2 + t;
        let h2 =       t3 -      t2;
        let h3 = -2. * t3 + 3. * t2;

        h0*p1 + h1*m0 + h2*m1 + h3*p2
    }

    pub fn update(&mut self) -> uv::Isometry3 {
        let t_pos   = self.t;
        let t_focus = t_pos + 0.002;
        let offset = uv::Vec3::unit_y() * -400.;

        let pos   = self.eval(t_pos)   + offset;
        let focus = self.eval(t_focus) + offset;

        let facing = (focus - pos).normalized();

        let pan = uv::Rotor3::from_rotation_between(
            uv::Vec3::unit_z(),
            (facing * uv::Vec3::new(1., 0., 1.)).normalized(),
        );

        let tilt = uv::Rotor3::from_rotation_between(
            uv::Vec3::unit_z(),
            (facing * uv::Vec3::new(0., 1., 0.) + uv::Vec3::unit_z()).normalized(),
        );

        let rotate = pan * tilt;

        self.t += 0.00015;
        uv::Isometry3::new(pos, rotate)
    }
}

