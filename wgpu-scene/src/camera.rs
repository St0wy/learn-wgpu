use cgmath::prelude::*;
use cgmath::{Matrix4, Point3, Vector3};
use winit::event::{DeviceEvent, ElementState, KeyboardInput, VirtualKeyCode, WindowEvent};

#[derive(Debug)]
pub struct Camera {
    pub position: Point3<f32>,
    pub aspect_ratio: f32,
    pub fov_y: f32,
    pub z_near: f32,
    pub z_far: f32,
    up: Vector3<f32>,
    front: Vector3<f32>,
    right: Vector3<f32>,
    pitch: f32,
    yaw: f32,
}

impl Camera {
    pub fn new(position: Point3<f32>, aspect_ratio: f32) -> Self {
        let mut camera = Camera {
            position,
            aspect_ratio,
            fov_y: 45.0,
            z_near: 0.001,
            z_far: 10000.0,
            up: Vector3::new(1.0, 0.0, 0.0),
            front: Vector3::new(0.0, 0.0, 1.0),
            right: Vector3::new(0.0, 1.0, 0.0),
            pitch: 0.0,
            yaw: f32::to_radians(-45.0),
        };
        camera.update_vectors();
        camera
    }

    pub fn build_view_projection_matrix(&self) -> Matrix4<f32> {
        let view = Matrix4::look_at_rh(self.position, self.position + self.front, self.up);
        let projection = cgmath::perspective(
            cgmath::Deg(self.fov_y),
            self.aspect_ratio,
            self.z_near,
            self.z_far,
        );

        OPENGL_TO_WGPU_MATRIX * projection * view
    }

    pub fn increment_pitch(&mut self, delta_pitch: f32) {
        self.pitch += delta_pitch;
        self.pitch = self
            .pitch
            .clamp(-89.0f32.to_radians(), 89.0f32.to_radians());
        self.update_vectors();
    }

    pub fn increment_yaw(&mut self, delta_yaw: f32) {
        self.yaw += delta_yaw;
        self.update_vectors();
    }

    pub fn set_fov_y(&mut self, fov_y: f32) {
        self.fov_y = fov_y.clamp(1.0f32.to_radians(), 45.0f32.to_radians());
    }

    fn update_vectors(&mut self) {
        self.front = Vector3 {
            x: f32::cos(self.pitch) * f32::cos(self.yaw),
            y: f32::sin(self.pitch),
            z: -f32::cos(self.pitch) * f32::sin(self.yaw),
        }
        .normalize();
        self.up = Vector3::new(0.0, 1.0, 0.0);
        self.right = self.front.cross(self.up).normalize();
    }
}

#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: Matrix4<f32> = Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.0,
    0.0, 0.0, 0.5, 1.0,
);

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    pub fn new() -> Self {
        Self {
            view_proj: Matrix4::identity().into(),
        }
    }

    pub fn update_view_proj(&mut self, camera: &Camera) {
        self.view_proj = camera.build_view_projection_matrix().into();
    }
}

pub struct CameraController {
    move_speed: f32,
    look_speed: f32,
    is_forward_pressed: bool,
    is_backward_pressed: bool,
    is_left_pressed: bool,
    is_right_pressed: bool,
    is_down_pressed: bool,
    is_up_pressed: bool,
    cursor_move: Option<(f32, f32)>,
}

impl CameraController {
    pub fn new(move_speed: f32, look_speed: f32) -> Self {
        Self {
            move_speed,
            look_speed,
            is_forward_pressed: false,
            is_backward_pressed: false,
            is_left_pressed: false,
            is_right_pressed: false,
            is_down_pressed: false,
            is_up_pressed: false,
            cursor_move: None,
        }
    }

    pub fn process_window_events(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state,
                        virtual_keycode: Some(keycode),
                        ..
                    },
                ..
            } => {
                let is_pressed = *state == ElementState::Pressed;
                match keycode {
                    VirtualKeyCode::W | VirtualKeyCode::Up => {
                        self.is_forward_pressed = is_pressed;
                        true
                    }
                    VirtualKeyCode::A | VirtualKeyCode::Left => {
                        self.is_left_pressed = is_pressed;
                        true
                    }
                    VirtualKeyCode::S | VirtualKeyCode::Down => {
                        self.is_backward_pressed = is_pressed;
                        true
                    }
                    VirtualKeyCode::D | VirtualKeyCode::Right => {
                        self.is_right_pressed = is_pressed;
                        true
                    }
                    VirtualKeyCode::LShift => {
                        self.is_down_pressed = is_pressed;
                        true
                    }
                    VirtualKeyCode::Space => {
                        self.is_up_pressed = is_pressed;
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    pub fn process_device_event(&mut self, event: &DeviceEvent) -> bool {
        match event {
            DeviceEvent::MouseMotion { delta } => {
                let delta_yaw = -delta.0.to_radians() as f32 * self.look_speed;
                let delta_pitch = -delta.1.to_radians() as f32 * self.look_speed;
                self.cursor_move = Some((delta_yaw, delta_pitch));

                true
            }
            _ => false,
        }
    }

    pub fn update_camera(&mut self, camera: &mut Camera) {
        if self.is_forward_pressed {
            camera.position += camera.front * self.move_speed;
        }
        if self.is_backward_pressed {
            camera.position += camera.front * -self.move_speed;
        }
        if self.is_right_pressed {
            camera.position += camera.right * self.move_speed;
        }
        if self.is_left_pressed {
            camera.position += camera.right * -self.move_speed;
        }
        if self.is_down_pressed {
            camera.position += camera.up * -self.move_speed;
        }
        if self.is_up_pressed {
            camera.position += camera.up * self.move_speed;
        }
        if let Some((delta_yaw, delta_pitch)) = self.cursor_move {
            camera.increment_yaw(delta_yaw);
            camera.increment_pitch(delta_pitch);
            self.cursor_move = None;
        }
    }
}
