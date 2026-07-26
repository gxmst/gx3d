use glam::Mat4;
use std::mem;

use super::bind_groups::ObjectUniforms;

const INITIAL_OBJECT_CAPACITY: usize = 1024;

/// Per-object uniforms shared through one dynamically-offset buffer.
///
/// `allocate` only appends to a CPU staging buffer; `flush` uploads the whole
/// frame's uniforms with a single `write_buffer` instead of one small copy per
/// object.
pub(crate) struct ObjectUniformState {
    buffer: wgpu::Buffer,
    pub(crate) bind_group: wgpu::BindGroup,
    capacity: usize,
    stride: usize,
    next_index: usize,
    staging: Vec<u8>,
}

impl ObjectUniformState {
    pub(crate) fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
        let raw_size = mem::size_of::<ObjectUniforms>();
        let alignment = device.limits().min_uniform_buffer_offset_alignment as usize;
        let stride = raw_size.div_ceil(alignment) * alignment;
        let capacity = INITIAL_OBJECT_CAPACITY;
        let (buffer, bind_group) = create_buffer_and_bind_group(device, layout, capacity, stride);
        Self {
            buffer,
            bind_group,
            capacity,
            stride,
            next_index: 0,
            staging: Vec::with_capacity(capacity * stride),
        }
    }

    pub(crate) fn ensure_capacity(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        count: usize,
    ) {
        if count <= self.capacity {
            return;
        }
        let new_capacity = count.next_power_of_two().max(INITIAL_OBJECT_CAPACITY);
        let (buffer, bind_group) =
            create_buffer_and_bind_group(device, layout, new_capacity, self.stride);
        self.buffer = buffer;
        self.bind_group = bind_group;
        self.capacity = new_capacity;
    }

    pub(crate) fn reset(&mut self) {
        self.next_index = 0;
        self.staging.clear();
    }

    /// Record one object's uniforms and return its dynamic offset. The data
    /// only reaches the GPU on the next `flush`.
    pub(crate) fn allocate(&mut self, model: Mat4) -> u32 {
        debug_assert!(
            self.next_index < self.capacity,
            "allocate() beyond capacity; call ensure_capacity() with the draw count first"
        );
        let index = self.next_index;
        self.next_index += 1;
        let offset = index * self.stride;
        let uniforms = ObjectUniforms::new(model);
        self.staging.resize(offset, 0);
        self.staging
            .extend_from_slice(bytemuck::cast_slice(&[uniforms]));
        // Pad to the aligned stride so the next allocation starts correctly.
        self.staging.resize(offset + self.stride, 0);
        offset as u32
    }

    /// Upload every uniform recorded since `reset` in one copy.
    pub(crate) fn flush(&self, queue: &wgpu::Queue) {
        if !self.staging.is_empty() {
            queue.write_buffer(&self.buffer, 0, &self.staging);
        }
    }
}

fn create_buffer_and_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    capacity: usize,
    stride: usize,
) -> (wgpu::Buffer, wgpu::BindGroup) {
    let raw_size = mem::size_of::<ObjectUniforms>();
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Object Uniform Buffer"),
        size: (capacity * stride) as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Object Bind Group"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &buffer,
                offset: 0,
                size: Some(core::num::NonZeroU64::new(raw_size as u64).unwrap()),
            }),
        }],
    });
    (buffer, bind_group)
}
