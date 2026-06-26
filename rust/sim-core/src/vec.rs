//! Heltalsvektor. Port av IVec2 i src/game/math/rect.hpp.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Vec2 {
    pub x: i32,
    pub y: i32,
}

impl Vec2 {
    #[inline]
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
    #[inline]
    pub fn zero() -> Self {
        Self { x: 0, y: 0 }
    }
    #[inline]
    pub fn add(self, r: Vec2) -> Vec2 {
        Vec2::new(self.x.wrapping_add(r.x), self.y.wrapping_add(r.y))
    }
    #[inline]
    pub fn sub(self, r: Vec2) -> Vec2 {
        Vec2::new(self.x.wrapping_sub(r.x), self.y.wrapping_sub(r.y))
    }
    #[inline]
    pub fn mul(self, s: i32) -> Vec2 {
        Vec2::new(self.x.wrapping_mul(s), self.y.wrapping_mul(s))
    }
    #[inline]
    pub fn div(self, s: i32) -> Vec2 {
        Vec2::new(self.x.wrapping_div(s), self.y.wrapping_div(s))
    }
    #[inline]
    pub fn neg(self) -> Vec2 {
        Vec2::new(self.x.wrapping_neg(), self.y.wrapping_neg())
    }
}
