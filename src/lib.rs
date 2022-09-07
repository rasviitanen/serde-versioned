/// A version id for your data.
/// Uses `u128` to be able to store a UUID.
pub struct Uuid<const N: u128>;

/// A version number for your data.
pub struct Num<const N: u32>;

/// A semantic version number for your data.
pub struct Sem<const A: u64, const B: u64, const C: u64>;

/// A version for your data.
pub struct Ver<T>(std::marker::PhantomData<T>);

/// The current version
pub struct Current;

/// Trait for an old deserialized value that can be converted into the current schema
pub trait FromVersion<V, Label = ()>
where
    for<'a> Self: serde::Deserialize<'a>,
{
    type VersionType: for<'a> serde::Deserialize<'a>;

    /// Converts the old data into the current type
    fn convert(v: Self::VersionType) -> Result<Self, Box<dyn std::error::Error>>;

    /// Tries to deserialize the FromVersion data by reference.
    fn deserialize_versioned<'de, Ds: serde::Deserializer<'de>>(
        content: &serde::__private::de::Content<'de>,
    ) -> Result<Self, Ds::Error> {
        use serde::Deserialize;
        use serde::__private::de::ContentRefDeserializer;

        if let Ok(res) =
            Self::VersionType::deserialize(ContentRefDeserializer::<Ds::Error>::new(content))
        {
            return Self::convert(res).map_err(serde::de::Error::custom);
        }

        Err(serde::de::Error::custom(
            "data did not match any version type",
        ))
    }
}

impl<T> FromVersion<Ver<Current>> for T
where
    for<'a> Self: serde::Deserialize<'a>,
{
    type VersionType = Self;

    fn convert(v: Self::VersionType) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(v)
    }
}

/// Different supported versions. Supports at most 10 simultaneous versions.
/// Use `LabeledVersions` if you need to support multiple version handlers for the same type.
#[allow(clippy::type_complexity)]
pub struct Versions<
    V0 = (),
    V1 = (),
    V2 = (),
    V3 = (),
    V4 = (),
    V5 = (),
    V6 = (),
    V7 = (),
    V8 = (),
    V9 = (),
>(std::marker::PhantomData<(V0, V1, V2, V3, V4, V5, V6, V7, V8, V9)>);

macro_rules! peel {
    (last { $last: tt, }; stack={$($stack: tt,)*}) => {
        impl_versions!(impl { $($stack,)* } for Versions);
    };
    (last { $first: tt, $($versions: tt,)+ }; stack={$($stack: tt,)*}) => {
        peel!(last { $($versions,)* }; stack={ $($stack,)* $first, });
    };
    (last { $first: tt, $($versions: tt,)+ }) => {
        peel!(last { $($versions,)* }; stack={ $first,});
    };
}

macro_rules! impl_versions {
    (impl { } for Versions) => {};
    (impl { $first: tt, } for Versions) => {};
    (impl { $first: tt, $($versions: tt,)* } for Versions) => {
        impl<$($versions,)*> Versions<Ver<Current>, $(Ver<$versions>,)*> {
            pub fn deserialize<'de, R, Ds: serde::Deserializer<'de>>(d: Ds) -> Result<R, Ds::Error>
            where
                R: FromVersion<Ver<Current>> $(+ FromVersion<Ver<$versions>>)*,
            {
                use serde::Deserialize;
                use serde::__private::de::Content;
                let content = Content::deserialize(d)?;
                FromVersion::<Ver<Current>>::deserialize_versioned::<Ds>(&content)
                    $(
                        .or_else(|_| FromVersion::<Ver<$versions>>::deserialize_versioned::<Ds>(&content))
                    )*
            }
        }

        peel!(last { $first, $($versions, )* });
    }
}

impl_versions!(impl { V0, V1, V2, V3, V4, V5, V6, V7, V8, V9, } for Versions);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_legacy() {
        struct OldString;
        #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
        struct Field3(u64);

        #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
        struct New {
            #[serde(deserialize_with = "Versions::<Ver<Current>, Ver<Uuid<1>>>::deserialize")]
            value: u32,

            #[serde(deserialize_with = "Versions::<Ver<Current>, Ver<OldString>>::deserialize")]
            value2: u32,

            #[serde(deserialize_with = "Versions::<Ver<Current>, Ver<Num<1>>>::deserialize")]
            value3: Field3,

            #[serde(deserialize_with = "Versions::<Ver<Current>, Ver<Sem<0, 0, 1>>>::deserialize")]
            value4: u32,
        }

        impl FromVersion<Ver<Sem<0, 0, 1>>> for u32 {
            type VersionType = String;

            fn convert(v: Self::VersionType) -> Result<Self, Box<dyn std::error::Error>> {
                Ok(v.parse::<u32>()? + 300)
            }
        }

        impl FromVersion<Ver<Uuid<1>>> for u32 {
            type VersionType = String;

            fn convert(v: Self::VersionType) -> Result<Self, Box<dyn std::error::Error>> {
                Ok(v.parse()?)
            }
        }

        impl FromVersion<Ver<OldString>> for u32 {
            type VersionType = String;

            fn convert(v: Self::VersionType) -> Result<Self, Box<dyn std::error::Error>> {
                Ok(v.parse::<Self>()? + 100)
            }
        }

        impl FromVersion<Ver<Num<1>>> for Field3 {
            type VersionType = String;

            fn convert(v: Self::VersionType) -> Result<Self, Box<dyn std::error::Error>> {
                Ok(Self(v.parse::<u64>()? + 200))
            }
        }

        #[derive(serde::Serialize, serde::Deserialize)]
        struct LegacyData {
            value: String,
            value2: String,
            value3: String,
            value4: String,
        }

        let legacy = serde_json::to_string(&LegacyData {
            value: String::from("100"),
            value2: String::from("100"),
            value3: String::from("100"),
            value4: String::from("100"),
        })
        .unwrap();

        assert_eq!(
            serde_json::from_str::<New>(&legacy).unwrap(),
            New {
                value: 100,
                value2: 200,
                value3: Field3(300),
                value4: 400,
            }
        );
    }
}
