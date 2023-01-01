use {
    crate::input::{Button, Bipole},
    ultraviolet as uv,
};

const TURN_SPEED: f32 = 0.00015;
const MOVE_SPEED: f32 = 50.;

pub struct DebugCam {
    pos: uv::Vec3,
    pan: f32,
    tilt: f32,

    ctrl_shift: [Bipole; 3],
    ctrl_fast: bool,
    ctrl_pan: f32,
    ctrl_tilt: f32,
}

impl DebugCam {
    pub fn new() -> Self {
        Self {
            pos: uv::Vec3::zero(),
            pan: 0.,
            tilt: 0.,
            ctrl_shift: Default::default(),
            ctrl_fast: false,
            ctrl_pan: 0.,
            ctrl_tilt: 0.,
        }
    }

    // fn input  (&mut E, &mut C)
    // fn update (&mut S, &C, E)

    pub fn update(&mut self) -> uv::Isometry3 {
        let pan_intent  = -self.ctrl_pan  as f32 * TURN_SPEED;
        let tilt_intent = -self.ctrl_tilt as f32 * TURN_SPEED;
        self.ctrl_pan = 0.;
        self.ctrl_tilt = 0.;
        self.pan = (self.pan + pan_intent).fract();
        self.tilt = (self.tilt + tilt_intent).clamp(-0.248, 0.248);

        let pan  = uv::Rotor3::from_rotation_xz(self.pan  * std::f32::consts::TAU);
        let tilt = uv::Rotor3::from_rotation_yz(self.tilt * std::f32::consts::TAU);
        let rotate = pan * tilt;

        let [dx, dy, dz] = self.ctrl_shift.map(|bp| bp.eval() as f32);
        let direction = rotate * uv::Vec3::new(dx, 0., dz) + uv::Vec3::new(0., dy, 0.);
        if direction.mag_sq() > 0.1 {
            self.pos += direction.normalized() * MOVE_SPEED * if self.ctrl_fast {5.} else {1.};
        }

        uv::Isometry3::new(self.pos, rotate)
    }

    pub fn button(&mut self, b: Button, down: bool) {
        match b {
            Button::Forward => self.ctrl_shift[2].pos(down),
            Button::Back    => self.ctrl_shift[2].neg(down),
            Button::Right   => self.ctrl_shift[0].pos(down),
            Button::Left    => self.ctrl_shift[0].neg(down),
            Button::Descend => self.ctrl_shift[1].pos(down),
            Button::Ascend  => self.ctrl_shift[1].neg(down),
            Button::Fast    => self.ctrl_fast = down,
            #[allow(unreachable_patterns)] _ => { }
        }
    }

    pub fn mouse(&mut self, delta: [f32; 2]) {
        self.ctrl_pan  += delta[0];
        self.ctrl_tilt += delta[1];
    }
}

