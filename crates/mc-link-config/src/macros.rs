//! Configuration definition macros for reducing boilerplate.

/// Macro for defining configuration structs with automatic trait implementations.
/// 
/// This macro generates:
/// - The struct definition with Serialize, Deserialize, Debug, Clone
/// - Default implementation using provided default values
/// - From<T> for config::Value implementation for integration with config crate
#[macro_export]
macro_rules! config_struct {
    (
        $(#[$struct_meta:meta])*
        $vis:vis struct $name:ident {
            $(
                $(#[$field_meta:meta])*
                $field_vis:vis $field_name:ident: $field_type:ty = $default_value:expr,
            )*
        }
    ) => {
        $(#[$struct_meta])*
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        $vis struct $name {
            $(
                $(#[$field_meta])*
                $field_vis $field_name: $field_type,
            )*
        }

        impl Default for $name {
            fn default() -> Self {
                Self {
                    $(
                        $field_name: $default_value,
                    )*
                }
            }
        }

        impl From<$name> for config::Value {
            fn from(val: $name) -> Self {
                use config::{ValueKind, Map};
                
                Self::new(
                    None,
                    ValueKind::Table(Map::from_iter(vec![
                        $(
                            (stringify!($field_name).to_string(), val.$field_name.into()),
                        )*
                    ])),
                )
            }
        }
    };
}

/// Macro for defining configuration enums with automatic trait implementations.
/// 
/// This macro generates:
/// - The enum definition with Debug, Clone, PartialEq, Eq, Serialize, Deserialize
/// - Default implementation using the specified default variant
/// - Display implementation for string conversion
/// - From<T> for config::Value implementation
#[macro_export]
macro_rules! config_enum {
    (
        $(#[$enum_meta:meta])*
        $vis:vis enum $name:ident {
            $(
                $(#[$variant_meta:meta])*
                $variant:ident $(= $value:expr)?,
            )*
        }
        default = $default_variant:ident
    ) => {
        $(#[$enum_meta])*
        #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
        $vis enum $name {
            $(
                $(#[$variant_meta])*
                $variant $(= $value)?,
            )*
        }

        impl Default for $name {
            fn default() -> Self {
                Self::$default_variant
            }
        }

        impl From<$name> for config::Value {
            fn from(val: $name) -> Self {
                use config::{Value, ValueKind};
                let s = match val {
                    $(
                        $name::$variant => stringify!($variant).to_lowercase(),
                    )*
                };
                Value::new(None, ValueKind::String(s))
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(
                        $name::$variant => write!(f, "{}", stringify!($variant).to_lowercase()),
                    )*
                }
            }
        }

        impl std::str::FromStr for $name {
            type Err = String;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s.to_lowercase().as_str() {
                    $(
                        stringify!($variant) => Ok(Self::$variant),
                    )*
                    _ => Err(format!("Invalid {} variant: {}", stringify!($name), s)),
                }
            }
        }
    };
}

/// Helper macro for creating nested config accessors.
/// 
/// This macro generates getter and mutable getter methods for nested configuration structs.
#[macro_export]
macro_rules! config_accessors {
    ($struct_name:ident, $($field_name:ident: $field_type:ty),*) => {
        impl $struct_name {
            $(
                paste::paste! {
                    /// Gets a reference to the configuration section.
                    pub fn $field_name(&self) -> &$field_type {
                        &self.$field_name
                    }

                    /// Gets a mutable reference to the configuration section.
                    pub fn [<$field_name _mut>](&mut self) -> &mut $field_type {
                        &mut self.$field_name
                    }
                }
            )*
        }
    };
}