pub mod multi_key_map {
    use std::{
        collections::HashMap, hash::Hash, mem::transmute, ops::{Deref, DerefMut}
    };

    pub struct Ref<V> {
        value: *mut V,
    }

    impl<V> Deref for Ref<V> {
        type Target = V;

        fn deref(&self) -> &Self::Target {
            unsafe { &*self.value }
        }
    }

    impl<V> DerefMut for Ref<V> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            unsafe { &mut *self.value }
        }
    }

    // impl<V> Drop for Ref<V> {
    //     fn drop(&mut self) {
    //         unsafe {
    //             Box::from_raw(self.value);
    //         }
    //     }
    // }

    impl<V> Clone for Ref<V> {
        fn clone(&self) -> Self {
            Self {
                value: self.value.clone(),
            }
        }
    }

    impl<V> Copy for Ref<V> {}

    impl<V> Ref<V> {
        pub fn new(mut value: V) -> Self {
            let boxed = Box::new(value);
            Self {
                value: Box::into_raw(boxed),
            }
        }
    }

    pub struct MultiMap<K, V>
    where
        K: Eq + PartialEq,
    {
        map: HashMap<K, V>,
    }

    impl<K, V> IntoIterator for MultiMap<K, V> 
    where
    K: Eq + PartialEq,
    {
        type Item = (K, V);
        type IntoIter = std::collections::hash_map::IntoIter<K, V>;

        fn into_iter(self) -> Self::IntoIter {
            self.map.into_iter()
        }
    }

    impl<'a, K, V> IntoIterator for &'a MultiMap<K, V> 
    where
    K: Eq + PartialEq,
    {
        type Item = (&'a K, &'a V);
        type IntoIter = std::collections::hash_map::Iter<'a, K, V>;

        fn into_iter(self) -> Self::IntoIter {
            self.map.iter()
        }
    }

    impl<'a, K, V> IntoIterator for &'a mut MultiMap<K, V> 
    where
    K: Eq + PartialEq,
    {
        type Item = (&'a K, &'a mut V);
        type IntoIter = std::collections::hash_map::IterMut<'a, K, V>;

        fn into_iter(self) -> Self::IntoIter {
            self.map.iter_mut()
        }
    }

    impl<K, V> MultiMap<K, V> 
    where
    K: Eq + PartialEq + Hash,
    {
        pub fn new() -> Self {
            Self {
                map: HashMap::new(),
            }
        }

        pub fn insert(&mut self, key: K, value: V) {
            self.map.insert(key, value);
        }

        pub fn get_mut(&mut self, key: &K)-> Option<&mut V> {
            self.map.get_mut(key)
        }
    }
}
