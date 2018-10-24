use std::fmt::{self, Display};

use lapin_async::message::Delivery;

pub struct MessageDelivery(Delivery);

impl MessageDelivery {
    pub fn new(d: Delivery) -> Self {
        MessageDelivery(d)
    }
}

impl Display for MessageDelivery {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let data = self.0.data.clone();
        let data2 = self.0.data.clone();
        f.write_str(&format!(
            "delivery_tag: {}, exchange: {}, routing_key: {}, redelivered: {}, props: {:?}, data: {}",
            self.0.delivery_tag,
            self.0.exchange,
            self.0.routing_key,
            self.0.redelivered,
            self.0.properties,
            String::from_utf8(data).unwrap_or(format!("{:?}", data2))
        ))
    }
}
