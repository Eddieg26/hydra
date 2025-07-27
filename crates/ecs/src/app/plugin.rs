use crate::AppBuilder;

#[allow(unused_variables)]
pub trait Plugin: 'static {
    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    /// Setup is called when the plugin is added to the app.
    /// It is used to register systems, resources, and other app components.
    fn setup(&mut self, app: &mut AppBuilder);

    /// Build is called when [AppBuilder::build] is called
    fn build(&mut self, app: &mut AppBuilder) {}

    /// Finish is called after all of a plugin's dependencies have been added and ran.
    fn finish(&mut self, app: &mut AppBuilder) {}
}

pub trait PluginCollection {
    fn add_plugin<P: Plugin>(&mut self, plugin: P) -> &mut Self;
}

pub trait PluginKit {
    fn get<P: PluginCollection>(self, plugins: &mut P);
}

impl<T: Plugin> PluginKit for T {
    fn get<P: PluginCollection>(self, plugins: &mut P) {
        plugins.add_plugin(self);
    }
}

#[macro_export]
macro_rules! impl_plugin_kit_for_tuples {
    ($(($($name:ident),*)),*)  => {
        $(
            #[allow(non_snake_case)]
            impl<$($name: PluginKit),+> PluginKit for ($($name),+) {
                fn get<Pc: PluginCollection>(self, plugins: &mut Pc) {
                    let ($($name),+) = self;
                    $(
                        $name.get(plugins);
                    )+
                }
            }
        )+
    };
}

impl_plugin_kit_for_tuples!((A, B));
impl_plugin_kit_for_tuples!((A, B, C));
impl_plugin_kit_for_tuples!((A, B, C, D));
impl_plugin_kit_for_tuples!((A, B, C, D, E));
impl_plugin_kit_for_tuples!((A, B, C, D, E, F));
impl_plugin_kit_for_tuples!((A, B, C, D, E, F, G));
impl_plugin_kit_for_tuples!((A, B, C, D, E, F, G, H));
impl_plugin_kit_for_tuples!((A, B, C, D, E, F, G, H, I));
impl_plugin_kit_for_tuples!((A, B, C, D, E, F, G, H, I, J));
impl_plugin_kit_for_tuples!((A, B, C, D, E, F, G, H, I, J, K));
impl_plugin_kit_for_tuples!((A, B, C, D, E, F, G, H, I, J, K, L));
impl_plugin_kit_for_tuples!((A, B, C, D, E, F, G, H, I, J, K, L, M));
impl_plugin_kit_for_tuples!((A, B, C, D, E, F, G, H, I, J, K, L, M, N));
impl_plugin_kit_for_tuples!((A, B, C, D, E, F, G, H, I, J, K, L, M, N, O));
impl_plugin_kit_for_tuples!((A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P));
impl_plugin_kit_for_tuples!((A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q));
