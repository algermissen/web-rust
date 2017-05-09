#!/bin/bash

PROMURI=https://github.com/prometheus/prometheus/releases/download/v1.6.1/prometheus-1.6.1.linux-amd64.tar.gz
PROM=prometheus-1.6.1.linux-amd64

GRAFANAURI=https://s3-us-west-2.amazonaws.com/grafana-releases/release/grafana_4.2.0_amd64.deb
GRAFANA=grafana_4.2.0_amd64.deb

curl -L $GRAFANAURI -o /opt/$GRAFANA
apt-get install -y adduser libfontconfig
dpkg -i /opt/$GRAFANA


curl -L $PROMURI -o /opt/$PROM.tar.gz
tar -xvzf /opt/$PROM.tar.gz -C /opt
ln -f -s /opt/$PROM /opt/prometheus

CFG=/opt/prometheus/prometheus.yml

cat > $CFG <<'DOC'
# my global config
global:
  scrape_interval:     5s # Set the scrape interval to every 15 seconds. Default is every 1 minute.
  evaluation_interval: 60s # Evaluate rules every 15 seconds. The default is every 1 minute.
  # scrape_timeout is set to the global default (10s).

  # Attach these labels to any time series or alerts when communicating with
  # external systems (federation, remote storage, Alertmanager).
  external_labels:
      monitor: 'codelab-monitor'

# Load rules once and periodically evaluate them according to the global 'evaluation_interval'.
rule_files:
  # - "first.rules"
  # - "second.rules"

# A scrape configuration containing exactly one endpoint to scrape:
# Here it's Prometheus itself.
scrape_configs:
  # The job name is added as a label `job=<job_name>` to any timeseries scraped from this config.
  - job_name: 'server'

    # metrics_path defaults to '/metrics'
    # scheme defaults to 'http'.

    static_configs:
      - targets: ['147.75.101.27:8080']
DOC

echo "Start prometheus:"
echo "/opt/prometheus/prometheus -config.file=/opt/prometheus/prometheus.yml"
