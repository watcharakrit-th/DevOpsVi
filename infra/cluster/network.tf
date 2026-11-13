data "aws_availability_zones" "available" {
  state = "available"
}

# 10.0.0.0/16 stays clear of k3s's own ranges (pods 10.42.0.0/16, services
# 10.43.0.0/16).
resource "aws_vpc" "main" {
  cidr_block           = "10.0.0.0/16"
  enable_dns_hostnames = true
  tags                 = { Name = "miniarcade" }
}

resource "aws_internet_gateway" "main" {
  vpc_id = aws_vpc.main.id
  tags   = { Name = "miniarcade" }
}

resource "aws_subnet" "public" {
  vpc_id            = aws_vpc.main.id
  cidr_block        = "10.0.1.0/24"
  availability_zone = data.aws_availability_zones.available.names[0]
  tags              = { Name = "miniarcade-public" }
}

resource "aws_route_table" "public" {
  vpc_id = aws_vpc.main.id

  route {
    cidr_block = "0.0.0.0/0"
    gateway_id = aws_internet_gateway.main.id
  }

  tags = { Name = "miniarcade-public" }
}

resource "aws_route_table_association" "public" {
  subnet_id      = aws_subnet.public.id
  route_table_id = aws_route_table.public.id
}

resource "aws_security_group" "node" {
  name        = "miniarcade-node"
  description = "MiniArcade node: HTTP from anywhere, SSH and the Kubernetes API from my_ip"
  vpc_id      = aws_vpc.main.id
  tags        = { Name = "miniarcade-node" }
}

# Intended: the site is public, over plain HTTP (an accepted risk).
#trivy:ignore:AWS-0107
resource "aws_vpc_security_group_ingress_rule" "http" {
  security_group_id = aws_security_group.node.id
  description       = "The site, plain HTTP"
  cidr_ipv4         = "0.0.0.0/0"
  ip_protocol       = "tcp"
  from_port         = 80
  to_port           = 80
}

resource "aws_vpc_security_group_ingress_rule" "ssh" {
  security_group_id = aws_security_group.node.id
  description       = "SSH for Ansible, from my_ip only"
  cidr_ipv4         = "${var.my_ip}/32"
  ip_protocol       = "tcp"
  from_port         = 22
  to_port           = 22
}

resource "aws_vpc_security_group_ingress_rule" "kube_api" {
  security_group_id = aws_security_group.node.id
  description       = "Kubernetes API for kubectl, from my_ip only"
  cidr_ipv4         = "${var.my_ip}/32"
  ip_protocol       = "tcp"
  from_port         = 6443
  to_port           = 6443
}

# Intended: the node pulls images, packages and charts from the internet.
#trivy:ignore:AWS-0104
resource "aws_vpc_security_group_egress_rule" "all" {
  security_group_id = aws_security_group.node.id
  description       = "All outbound"
  cidr_ipv4         = "0.0.0.0/0"
  ip_protocol       = "-1"
}
