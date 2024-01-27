use crate::recycle::holder::RecycleHolder;
use enum_key::{EnumKey, KeyableEnum};
use std::collections::HashMap;
use tokio::sync::mpsc;

#[macro_export]
macro_rules! recv_recycling {
    ($prod:expr, $holder:ident, $enum_type:ident::$key:ident {$($vars:ident),*}) => {
        let mut $holder = ::anyhow::Context::context($prod.recv_recycling(<$enum_type as ::enum_key::KeyableEnum>::Key::$key).await, "Failed to receive recycling")?;
        let $enum_type::$key {$($vars),*} = ::std::ops::DerefMut::deref_mut(&mut $holder) else { panic!("Returned wrong key") };
    };
    ($prod:expr, $holder:ident, $enum_type:ident::$key:ident ($($vars:ident),*)) => {
        let mut $holder = ::anyhow::Context::context($prod.recv_recycling(<$enum_type as ::enum_key::KeyableEnum>::Key::$key).await, "Failed to receive recycling")?;
        let $enum_type::$key ($($vars),*) = ::std::ops::DerefMut::deref_mut(&mut $holder) else { panic!("Returned wrong key") };
    };
    ($prod:expr, $holder:ident, $enum_type:ident::$key:ident {$($vars:ident),*}!) => {
        let mut $holder = $prod.recv_recycling(<$enum_type as ::enum_key::KeyableEnum>::Key::$key).await.unwrap();
        let $enum_type::$key {$($vars),*} = ::std::ops::DerefMut::deref_mut(&mut $holder) else { panic!("Returned wrong key") };
    };
    ($prod:expr, $holder:ident, $enum_type:ident::$key:ident ($($vars:ident),*)!) => {
        let mut $holder = $prod.recv_recycling(<$enum_type as ::enum_key::KeyableEnum>::Key::$key).await.unwrap();
        let $enum_type::$key ($($vars),*) = ::std::ops::DerefMut::deref_mut(&mut $holder) else { panic!("Returned wrong key") };
    };
}

pub struct EnumRecycleProducer<T: Send + KeyableEnum> {
    recycle_rx: HashMap<T::Key, ReceiverHolder<T>>,
    data_tx: mpsc::Sender<T>,
}

pub struct EnumRecycleConsumer<T: Send + KeyableEnum> {
    data_rx: ReceiverHolder<T>,
    recycle_tx: HashMap<T::Key, mpsc::Sender<T>>,
}

struct ReceiverHolder<T: Send> {
    not_consumed: Option<T>,
    rx: mpsc::Receiver<T>,
}

pub async fn enum_recycler<T: Send + KeyableEnum>(
    objs: Vec<T>,
) -> (EnumRecycleProducer<T>, EnumRecycleConsumer<T>) {
    let (data_tx, data_rx) = mpsc::channel(objs.len());

    let mut sorted = HashMap::new();
    for obj in objs {
        sorted
            .entry(obj.get_enum_key())
            .or_insert_with(|| vec![])
            .push(obj);
    }

    if sorted.len() != T::Key::VALUES.len() {
        let mut missing_keys: Vec<_> = T::Key::VALUES.iter().copied().collect();
        missing_keys.retain(|key| !sorted.contains_key(key));
        let missing_keys_str = missing_keys
            .iter()
            .map(|key| format!("{:?}", key))
            .collect::<Vec<_>>()
            .join(", ");

        panic!(
            "Attempted to construct enum recycler but missing recyclables for [{}] variants",
            missing_keys_str
        );
    }

    let mut recycle_rx = HashMap::new();
    let mut recycle_tx = HashMap::new();
    for (key, vec) in sorted {
        let (tx, rx) = mpsc::channel(vec.len());
        let holder = ReceiverHolder {
            not_consumed: None,
            rx,
        };

        for obj in vec {
            tx.send(obj).await.unwrap();
        }

        recycle_rx.insert(key, holder);
        recycle_tx.insert(key, tx);
    }

    (
        EnumRecycleProducer {
            recycle_rx,
            data_tx,
        },
        EnumRecycleConsumer {
            data_rx: ReceiverHolder {
                not_consumed: None,
                rx: data_rx,
            },
            recycle_tx,
        },
    )
}

impl<T: Send + KeyableEnum> EnumRecycleProducer<T> {
    pub async fn recv_recycling(&mut self, key: T::Key) -> Option<RecycleHolder<T>> {
        let holder = self
            .recycle_rx
            .get_mut(&key)
            .expect(&format!("Missing receiver for {:?}", key));

        if let Some(obj) = holder.not_consumed.take() {
            return Some(RecycleHolder::new(
                &mut holder.not_consumed,
                Some(obj),
                self.data_tx.clone(),
            ));
        }

        if let Some(obj) = holder.rx.recv().await {
            return Some(RecycleHolder::new(
                &mut holder.not_consumed,
                Some(obj),
                self.data_tx.clone(),
            ));
        }

        None
    }
}

impl<T: Send + KeyableEnum> EnumRecycleConsumer<T> {
    pub fn recv_data_blocking(&mut self) -> Option<RecycleHolder<T>> {
        if let Some(obj) = self.data_rx.not_consumed.take() {
            let key = obj.get_enum_key();
            let tx = self
                .recycle_tx
                .get(&key)
                .expect(&format!("Missing sender for {:?}", key));
            return Some(RecycleHolder::new(
                &mut self.data_rx.not_consumed,
                Some(obj),
                tx.clone(),
            ));
        }

        if let Some(obj) = self.data_rx.rx.blocking_recv() {
            let key = obj.get_enum_key();
            let tx = self
                .recycle_tx
                .get(&key)
                .expect(&format!("Missing sender for {:?}", key));
            return Some(RecycleHolder::new(
                &mut self.data_rx.not_consumed,
                Some(obj),
                tx.clone(),
            ));
        }

        None
    }
}

#[cfg(test)]
mod testing {
    use crate::recycle::r#enum::enum_recycler;
    use enum_key::KeyableEnum;
    use std::ops::DerefMut;

    #[derive(KeyableEnum)]
    pub enum MyEnum {
        Audio(u8, u16),
        Video(u32),
    }

    #[tokio::test]
    async fn test2() {
        let (mut tx, _rx) = enum_recycler(vec![MyEnum::Audio(13, 14), MyEnum::Video(42)]).await;
        recv_recycling!(tx, holder, MyEnum::Audio(audio, audio2)!);
        assert_eq!(*audio, 13);
        assert_eq!(*audio2, 14);
        *audio = 2;
        holder.send().await.unwrap();
    }
}
