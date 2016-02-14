use threadpool::Pooled;

#[derive(Debug)]
pub enum QTask {
    Nothing
}

pub struct RQueue;
impl Pooled<QTask, ()> for RQueue {
    fn func (task: QTask) {
        println!("{:?}", task);
    }
}
