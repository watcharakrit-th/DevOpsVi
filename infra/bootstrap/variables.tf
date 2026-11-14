variable "region" {
  description = "AWS region for the whole lab."
  type        = string
  default     = "ap-southeast-1"
}

variable "alert_email" {
  description = "Receives the budget emails. Set in ~/.miniarcade/bootstrap.tfvars, never in Git."
  type        = string

  validation {
    condition     = can(regex("^[^@\\s]+@[^@\\s]+\\.[^@\\s]+$", var.alert_email))
    error_message = "alert_email must be an email address."
  }
}

# GitHub reports it with:
#   gh api repos/watcharakrit-th/DevOpsVi/actions/oidc/customization/sub
variable "github_sub_prefix" {
  description = "Start of the OIDC subject GitHub issues for this repository (owner@id/repo@id)."
  type        = string
  default     = "repo:watcharakrit-th@247355252/DevOpsVi@1455186502"
}
