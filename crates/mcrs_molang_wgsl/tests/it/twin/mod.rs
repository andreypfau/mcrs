use mcrs_molang::{PureInstr, PureProgram, Register};

mod gpu_math;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TwinRun {
    pub value: f32,
    pub stayed_finite: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Twin {
    registers: Vec<f32>,
}

impl Twin {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn run(&mut self, program: &PureProgram, inputs: &[f32]) -> TwinRun {
        self.registers.clear();
        self.registers.resize(program.registers(), 0.0);
        let mut stayed_finite = true;
        let code = program.instructions();
        let mut pc = 0usize;
        while let Some(instr) = code.get(pc) {
            pc += 1;
            let (dst, value) = match *instr {
                PureInstr::Constant { dst, value } => (dst, gpu_math::flush(value)),
                PureInstr::Input { dst, input } => {
                    let value = inputs.get(usize::from(input)).copied().unwrap_or(0.0);
                    (dst, gpu_math::flush(value))
                }
                PureInstr::Unary { function, dst, a } => {
                    (dst, gpu_math::unary(function, self.get(a)))
                }
                PureInstr::Binary {
                    function,
                    dst,
                    a,
                    b,
                } => (dst, gpu_math::binary(function, self.get(a), self.get(b))),
                PureInstr::Ternary {
                    function,
                    dst,
                    a,
                    b,
                    c,
                } => (
                    dst,
                    gpu_math::ternary(function, self.get(a), self.get(b), self.get(c)),
                ),
                PureInstr::Select {
                    dst,
                    condition,
                    then,
                    otherwise,
                } => {
                    let truthy = self.get(condition) != 0.0;
                    (dst, self.get(if truthy { then } else { otherwise }))
                }
                PureInstr::Move { dst, src } => (dst, self.get(src)),
                PureInstr::If {
                    condition,
                    otherwise,
                } => {
                    if self.get(condition) == 0.0 {
                        pc = otherwise as usize + 1;
                    }
                    continue;
                }
                PureInstr::Else { end } => {
                    pc = end as usize + 1;
                    continue;
                }
                PureInstr::Return { src } => {
                    return TwinRun {
                        value: gpu_math::pose_clamp(self.get(src)),
                        stayed_finite,
                    };
                }
                _ => continue,
            };
            stayed_finite &= value.is_finite();
            if let Some(slot) = self.registers.get_mut(dst.index()) {
                *slot = value;
            }
        }
        TwinRun {
            value: 0.0,
            stayed_finite,
        }
    }

    fn get(&self, register: Register) -> f32 {
        self.registers.get(register.index()).copied().unwrap_or(0.0)
    }
}
