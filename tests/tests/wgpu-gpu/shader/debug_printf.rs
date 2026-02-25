use wgpu::{
    include_wgsl, CommandEncoderDescriptor, ComputePassDescriptor, ComputePipelineDescriptor,
    Features, Limits, PipelineCompilationOptions, PipelineLayoutDescriptor, PollType,
};

use wgpu_test::{gpu_test, GpuTestConfiguration, GpuTestInitializer, TestParameters};

pub fn all_tests(vec: &mut Vec<GpuTestInitializer>) {
    vec.push(DEBUG_PRINTF);
}

#[gpu_test]
static DEBUG_PRINTF: GpuTestConfiguration = GpuTestConfiguration::new()
    .parameters(
        TestParameters::default()
            .features(Features::DEBUG_PRINTF)
            .limits(Limits::default())
            // fxc is the only DX compiler with printf support
            .force_fxc(true),
    )
    .run_sync(|ctx| {
        // SAFETY: WGPU tests are run one at a time
        unsafe {
            std::env::set_var("VK_LAYER_PRINTF_ENABLE", "1");
            std::env::set_var("VK_LAYER_PATH", "/usr/local/share/vulkan/explicit_layer.d");
        }

        let pll = ctx
            .device
            .create_pipeline_layout(&PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[],
                immediate_size: 0,
            });

        let sm = ctx
            .device
            .create_shader_module(include_wgsl!("debug_printf.wgsl"));

        let pipeline = ctx
            .device
            .create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some("debugprintf"),
                layout: Some(&pll),
                compilation_options: PipelineCompilationOptions::default(),
                module: &sm,
                entry_point: Some("main"),
                cache: None,
            });

        // -- Run test --

        let mut encoder = ctx
            .device
            .create_command_encoder(&CommandEncoderDescriptor::default());

        {
            let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor::default());
            cpass.set_pipeline(&pipeline);
        }

        ctx.queue.submit(Some(encoder.finish()));

        ctx.device.poll(PollType::wait_indefinitely()).unwrap();

        // todo read stdout or system specific output location
        // output should be "Hello world 1" 64 times
    });
