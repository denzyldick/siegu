FROM caddy:2.9-alpine

ARG SIEGU_GA_ID=__GA_MEASUREMENT_ID__
ARG SIEGU_STRIPE_MONTHLY=__STRIPE_PRO_PAYMENT_LINK_MONTHLY__
ARG SIEGU_STRIPE_YEARLY=__STRIPE_PRO_PAYMENT_LINK_YEARLY__

COPY public /srv
COPY deploy/Caddyfile /etc/caddy/Caddyfile

# Bake the GA4 Measurement ID into the tracker (placeholder / unset keeps GA off).
RUN sed -i "s|__GA_MEASUREMENT_ID__|${SIEGU_GA_ID}|g" /srv/js/main.js
# Bake the Stripe Payment Links into the Pro checkout buttons (placeholders keep buttons inert).
RUN sed -i "s|__STRIPE_PRO_PAYMENT_LINK_MONTHLY__|${SIEGU_STRIPE_MONTHLY}|g" /srv/js/main.js
RUN sed -i "s|__STRIPE_PRO_PAYMENT_LINK_YEARLY__|${SIEGU_STRIPE_YEARLY}|g" /srv/js/main.js

EXPOSE 80 443