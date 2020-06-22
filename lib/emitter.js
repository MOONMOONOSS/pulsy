const { EventEmitter } = require('events');
const { Consumer } = require('../native/index.node');

class ConsumerEventEmitter extends EventEmitter {
  constructor(address, uuid) {
    super();

    this.channel = new Consumer(address, uuid);

    this.isShutdown = false;

    const loop = () => {
      if (this.isShutdown) {
        this.channel.close();

        return;
      }
      
      this.channel.poll((err, ev) => {
        if (err) {
          console.error(err);
          this.emit('error', err);
        }
        else if (ev) {
          const { event, ...data } = ev;

          this.emit(event, data);
        }

        setImmediate(loop);
      });
    };

    setImmediate(loop);
  }

  close() {
    this.isShutdown = true;
    return this;
  }
}

module.exports = ConsumerEventEmitter;
