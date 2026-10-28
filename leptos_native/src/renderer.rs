use anyrender::{
    CompositeAlphaMode, RenderContext, RendererConfig, WindowRenderer,
};
use peniko::Color;
use std::{any::Any, cell::RefCell, rc::Rc, sync::Arc};

cfg_if::cfg_if! {
    if #[cfg(feature = "vello")] {
        use anyrender_vello::{
            VelloRendererOptions as InnerRendererOptions, VelloWindowRenderer as InnerRenderer,
        };
        pub use anyrender_vello::wgpu::{Features, Limits};
    } else if #[cfg(feature = "vello-hybrid")] {
        use anyrender_vello_hybrid::{
            VelloHybridRendererOptions as InnerRendererOptions, VelloHybridWindowRenderer as InnerRenderer,
        };
        pub use anyrender_vello_hybrid::wgpu::{Features, Limits};
    } else if #[cfg(feature = "skia")] {
        use anyrender_skia::SkiaWindowRenderer as InnerRenderer;
    } else if #[cfg(feature = "skia-raster-base")] {
        use anyrender_skia::raster::SkiaRasterWindowRenderer as InnerRenderer;
    } else {
        use anyrender_vello_cpu::VelloCpuWindowRenderer as InnerRenderer;
    }
}

/// Renderer configuration for [`LeptosNativeWindowRenderer`].
///
/// Fields that only apply to the GPU-backed `vello`/`vello-hybrid` renderers
/// (`features`/`limits`) are only present when one of those features is enabled.
#[derive(Default)]
pub struct RendererOptions {
    /// Base (background) color used to clear each frame.
    pub base_color: Option<Color>,
    /// Alpha mode used when compositing the window surface.
    pub alpha_mode: Option<CompositeAlphaMode>,
    /// wgpu features to request from the GPU adapter.
    #[cfg(any(feature = "vello", feature = "vello-hybrid"))]
    pub features: Option<Features>,
    /// wgpu limits to request from the GPU adapter.
    #[cfg(any(feature = "vello", feature = "vello-hybrid"))]
    pub limits: Option<Limits>,
}

/// Feature-selected window renderer. See [`crate::launch`] for backend priority.
#[derive(Clone)]
pub struct LeptosNativeWindowRenderer {
    inner: Rc<RefCell<InnerRenderer>>,
}

impl Default for LeptosNativeWindowRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl LeptosNativeWindowRenderer {
    pub fn new() -> Self {
        Self::with_options(RendererOptions::default())
    }

    #[cfg(any(feature = "vello-hybrid", feature = "vello"))]
    pub fn with_features_and_limits(
        features: Option<Features>,
        limits: Option<Limits>,
    ) -> Self {
        Self::with_options(RendererOptions {
            features,
            limits,
            ..Default::default()
        })
    }

    /// Build a renderer from the given [`RendererOptions`].
    ///
    /// `base_color` and `alpha_mode` are forwarded to the active renderer via
    /// [`anyrender::RendererConfig`]; renderers that don't support a particular
    /// option simply ignore it. `features`/`limits` are only applied by the
    /// GPU-backed `vello`/`vello-hybrid` renderers.
    pub fn with_options(options: RendererOptions) -> Self {
        let mut config = RendererConfig::default();
        config.base_color = options.base_color;
        config.composite_alpha_mode = options.alpha_mode;

        cfg_if::cfg_if! {
            if #[cfg(any(feature = "vello", feature = "vello-hybrid"))] {
                let mut inner_options: InnerRendererOptions = config.into();
                if let Some(features) = options.features {
                    inner_options = inner_options.features(features);
                }
                if let Some(limits) = options.limits {
                    inner_options = inner_options.limits(limits);
                }
                let inner_renderer = InnerRenderer::with_options(inner_options);
            } else {
                let inner_renderer = InnerRenderer::with_options(config);
            }
        }

        Self::with_inner_renderer(inner_renderer)
    }

    fn with_inner_renderer(inner_renderer: InnerRenderer) -> Self {
        Self {
            inner: Rc::new(RefCell::new(inner_renderer)),
        }
    }
}

impl RenderContext for LeptosNativeWindowRenderer {
    fn try_register_custom_resource(
        &mut self,
        resource: Box<dyn Any>,
    ) -> Result<anyrender::ResourceId, anyrender::RegisterResourceError> {
        self.inner
            .borrow_mut()
            .try_register_custom_resource(resource)
    }

    fn unregister_resource(&mut self, resource_id: anyrender::ResourceId) {
        self.inner.borrow_mut().unregister_resource(resource_id)
    }

    fn renderer_specific_context(&self) -> Option<Box<dyn Any>> {
        self.inner.borrow_mut().renderer_specific_context()
    }
}
impl WindowRenderer for LeptosNativeWindowRenderer {
    type ScenePainter<'a>
        = <InnerRenderer as WindowRenderer>::ScenePainter<'a>
    where
        Self: 'a;

    fn resume<F: FnOnce() + 'static>(
        &mut self,
        window: Arc<dyn anyrender::WindowHandle>,
        width: u32,
        height: u32,
        on_ready: F,
    ) {
        self.inner
            .borrow_mut()
            .resume(window, width, height, on_ready)
    }

    fn complete_resume(&mut self) -> bool {
        self.inner.borrow_mut().complete_resume()
    }

    fn suspend(&mut self) {
        self.inner.borrow_mut().suspend()
    }

    fn is_active(&self) -> bool {
        self.inner.borrow().is_active()
    }

    fn is_pending(&self) -> bool {
        self.inner.borrow().is_pending()
    }

    fn set_size(&mut self, width: u32, height: u32) {
        self.inner.borrow_mut().set_size(width, height)
    }

    fn render<F: FnOnce(&mut Self::ScenePainter<'_>)>(&mut self, draw_fn: F) {
        self.inner.borrow_mut().render(draw_fn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_renderer_starts_inactive() {
        let renderer = LeptosNativeWindowRenderer::default();
        assert!(!renderer.is_active());
        assert!(!renderer.clone().is_active());
    }

    #[test]
    fn accepts_background_and_alpha_options() {
        let renderer =
            LeptosNativeWindowRenderer::with_options(RendererOptions {
                base_color: Some(Color::WHITE),
                alpha_mode: Some(CompositeAlphaMode::Auto),
                ..Default::default()
            });
        assert!(!renderer.is_active());
    }
}
