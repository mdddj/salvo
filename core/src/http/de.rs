use std::borrow::Cow;
use std::collections::HashMap;
use std::hash::Hash;

use multimap::MultiMap;
pub(crate) use serde::de::value::{Error, MapDeserializer, SeqDeserializer};
use serde::de::{
    Deserialize, DeserializeSeed, Deserializer, EnumAccess, Error as DeError, IntoDeserializer, VariantAccess, Visitor,
};
use serde::forward_to_deserialize_any;

pub(crate) fn from_str_map<'de, T, K, V>(input: &'de HashMap<K, V>) -> Result<T, Error>
where
    T: Deserialize<'de>,
    K: AsRef<str>,
    V: AsRef<str>,
{
    let iter = input
        .iter()
        .map(|(k, v)| (CowValue(Cow::from(k.as_ref())), CowValue(Cow::from(v.as_ref()))));
    T::deserialize(MapDeserializer::new(iter))
}
pub(crate) fn from_str_multi_map<'de, T, K, V>(input: &'de MultiMap<K, V>) -> Result<T, Error>
where
    T: Deserialize<'de>,
    K: AsRef<str> + Hash + std::cmp::Eq,
    V: AsRef<str> + std::cmp::Eq,
{
    let iter = input.iter_all().map(|(k, v)| {
        (
            CowValue(Cow::from(k.as_ref())),
            VecValue(v.iter().map(|v| CowValue(Cow::from(v.as_ref()))).collect()),
        )
    });
    T::deserialize(MapDeserializer::new(iter))
}

macro_rules! forward_cow_value_parsed_value {
    ($($ty:ident => $method:ident,)*) => {
        $(
            fn $method<V>(self, visitor: V) -> Result<V::Value, Self::Error>
                where V: Visitor<'de>
            {
                match self.0.parse::<$ty>() {
                    Ok(val) => val.into_deserializer().$method(visitor),
                    Err(e) => Err(DeError::custom(e))
                }
            }
        )*
    }
}

macro_rules! forward_vec_value_parsed_value {
    ($($ty:ident => $method:ident,)*) => {
        $(
            fn $method<V>(self, visitor: V) -> Result<V::Value, Self::Error>
                where V: Visitor<'de>
            {
                if let Some(item) = self.0.get(0).to_owned() {
                    match item.0.parse::<$ty>() {
                        Ok(val) => val.into_deserializer().$method(visitor),
                        Err(e) => Err(DeError::custom(e))
                    }
                } else {
                    Err(DeError::custom("expected vec not empty"))
                }
            }
        )*
    }
}

struct CowValue<'de>(Cow<'de, str>);
impl<'de> IntoDeserializer<'de> for CowValue<'de> {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self::Deserializer {
        self
    }
}

impl<'de> Deserializer<'de> for CowValue<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.0 {
            Cow::Borrowed(value) => visitor.visit_borrowed_str(value),
            Cow::Owned(value) => visitor.visit_string(value),
        }
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_some(self)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_enum(ValueEnumAccess(self.0))
    }

    fn deserialize_newtype_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    forward_to_deserialize_any! {
        char
        str
        string
        unit
        bytes
        byte_buf
        unit_struct
        tuple_struct
        struct
        identifier
        tuple
        ignored_any
        seq
        map
    }

    forward_cow_value_parsed_value! {
        bool => deserialize_bool,
        u8 => deserialize_u8,
        u16 => deserialize_u16,
        u32 => deserialize_u32,
        u64 => deserialize_u64,
        i8 => deserialize_i8,
        i16 => deserialize_i16,
        i32 => deserialize_i32,
        i64 => deserialize_i64,
        f32 => deserialize_f32,
        f64 => deserialize_f64,
    }
}

struct VecValue<'de>(Vec<CowValue<'de>>);
impl<'de> IntoDeserializer<'de> for VecValue<'de> {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self::Deserializer {
        self
    }
}

impl<'de> Deserializer<'de> for VecValue<'de> {
    type Error = Error;

    fn deserialize_any<V>(mut self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if !self.0.is_empty() {
            let item = self.0.remove(0);
            item.deserialize_any(visitor)
        } else {
            Err(DeError::custom("expected vec not empty"))
        }
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_some(self)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if let Some(item) = self.0.get(0) {
            visitor.visit_enum(ValueEnumAccess(item.0.clone()))
        } else {
            Err(DeError::custom("expected vec not empty"))
        }
    }

    fn deserialize_newtype_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_tuple_struct<V>(self, _name: &'static str, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }
    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }
    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_seq(SeqDeserializer::new(self.0.into_iter()))
    }

    forward_to_deserialize_any! {
        char
        str
        string
        unit
        bytes
        byte_buf
        unit_struct
        struct
        identifier
        ignored_any
        map
    }

    forward_vec_value_parsed_value! {
        bool => deserialize_bool,
        u8 => deserialize_u8,
        u16 => deserialize_u16,
        u32 => deserialize_u32,
        u64 => deserialize_u64,
        i8 => deserialize_i8,
        i16 => deserialize_i16,
        i32 => deserialize_i32,
        i64 => deserialize_i64,
        f32 => deserialize_f32,
        f64 => deserialize_f64,
    }
}

struct ValueEnumAccess<'de>(Cow<'de, str>);

impl<'de> EnumAccess<'de> for ValueEnumAccess<'de> {
    type Error = Error;
    type Variant = UnitOnlyVariantAccess;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        let variant = seed.deserialize(self.0.into_deserializer())?;
        Ok((variant, UnitOnlyVariantAccess))
    }
}

struct UnitOnlyVariantAccess;

impl<'de> VariantAccess<'de> for UnitOnlyVariantAccess {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, _seed: T) -> Result<T::Value, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        Err(DeError::custom("expected unit variant"))
    }

    fn tuple_variant<V>(self, _len: usize, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        Err(DeError::custom("expected unit variant"))
    }

    fn struct_variant<V>(self, _fields: &'static [&'static str], _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        Err(DeError::custom("expected unit variant"))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde::Deserialize;
    use multimap::MultiMap;

    #[tokio::test]
    async fn test_de_str_map() {
        #[derive(Deserialize, Eq, PartialEq, Debug)]
        struct User {
            name: String,
            age: u8,
        }

        let mut data: HashMap<String, String> = HashMap::new();
        data.insert("age".into(), "10".into());
        data.insert("name".into(), "hello".into());
        let user: User = super::from_str_map(&data).unwrap();
        assert_eq!(user.age, 10);
    }

    #[tokio::test]
    async fn test_de_str_multi_map() {
        #[derive(Deserialize, Eq, PartialEq, Debug)]
        struct User<'a> {
            id: i64,
            name: &'a str,
            age: u8,
            friends: (String, String, i64),
            kids: Vec<String>,
            lala: Vec<i64>,
        }
        
        let mut map = MultiMap::new();

        map.insert("id", "42");
        map.insert("name", "Jobs");
        map.insert("age", "100");
        map.insert("friends", "100");
        map.insert("friends", "200");
        map.insert("friends", "300");
        map.insert("kids", "aaa");
        map.insert("kids", "bbb");
        map.insert("kids", "ccc");
        map.insert("lala", "600");
        map.insert("lala", "700");

        let user: User = super::from_str_multi_map(&map).unwrap();
        assert_eq!(user.id, 42);
    }
}
