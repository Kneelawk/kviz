use crate::recycle::holder::RecycleHolder;
use tokio::sync::mpsc;

pub struct RecycleProducer<T: Send> {
    not_consumed: Option<T>,
    recycle_rx: mpsc::Receiver<T>,
    data_tx: mpsc::Sender<T>,
}

pub struct RecycleConsumer<T: Send> {
    not_consumed: Option<T>,
    data_rx: mpsc::Receiver<T>,
    recycle_tx: mpsc::Sender<T>,
}

pub async fn recycler<T: Send>(objs: Vec<T>) -> (RecycleProducer<T>, RecycleConsumer<T>) {
    let (data_tx, data_rx) = mpsc::channel(objs.len());
    let (recycle_tx, recycle_rx) = mpsc::channel(objs.len());

    for obj in objs {
        recycle_tx.send(obj).await.unwrap();
    }

    (
        RecycleProducer {
            not_consumed: None,
            recycle_rx,
            data_tx,
        },
        RecycleConsumer {
            not_consumed: None,
            data_rx,
            recycle_tx,
        },
    )
}

impl<T: Send> RecycleProducer<T> {
    pub fn recv_recycling_blocking(&mut self) -> Option<RecycleHolder<T>> {
        if let Some(obj) = self.not_consumed.take() {
            return Some(RecycleHolder::new(
                &mut self.not_consumed,
                Some(obj),
                self.data_tx.clone(),
            ));
        }

        if let Some(obj) = self.recycle_rx.blocking_recv() {
            return Some(RecycleHolder::new(
                &mut self.not_consumed,
                Some(obj),
                self.data_tx.clone(),
            ));
        }

        None
    }
}

impl<T: Send> RecycleConsumer<T> {
    pub async fn recv_data(&mut self) -> Option<RecycleHolder<T>> {
        if let Some(obj) = self.not_consumed.take() {
            return Some(RecycleHolder::new(
                &mut self.not_consumed,
                Some(obj),
                self.recycle_tx.clone(),
            ));
        }

        if let Some(obj) = self.data_rx.recv().await {
            return Some(RecycleHolder::new(
                &mut self.not_consumed,
                Some(obj),
                self.recycle_tx.clone(),
            ));
        }

        None
    }
}
