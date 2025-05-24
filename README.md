# Oxifed Activitypub daemons

This repo contains the Daemons for serving domains on the Activitypub network.
It is based on the following components.

The main crate contains Message types and Cleint

# Domainservd
This daemon serves the inbox, webfinger and Outbox Endpoints it is the main daemon external services connect to.
It is also the daemon any internal app connects to. Internal Apps can Publish Objects as actors or for Actors. Actors are linked to domains all
of that is saved to mongodb. RabbitMQ is used as the communication channel for all the internal messages. Both for domainservd to receive
Objects and People or actors but also for domainservd to contact its workers. Everytime a message is sent to a actors inbox or to the shared inbox it is sent to the `INCOMMING_EXCHANGE` where it can be analyzed and potentially filtered by worker daemons. If a Message is received which gets identified as answer to a action performed by a domainservd daemon then domainservd processes this message itself.

# Publisherd
This listens for Activities on the `EXCHANGE_ACTIVITYPUB_PUBLISH` channel and handles all the complicated activitypub publishing logic defined in https://www.w3.org/TR/activitypub/

# Oxiadm
A small cli to publish notes and create and administrate people records. With it you can also follow other activitypub accounts and like or boost notes. Its a tiny CLI app to allow for basic smoke testing of the whole system when it's connected to the internet and start interaction with other Activitypub servers. It cannot display any notes or Objects as that is outside of it's scope.
